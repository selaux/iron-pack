#![cfg_attr(feature = "unstable", feature(test))]
//! Compression middleware for Iron. This crate lets you automatically compress iron responses
//! by providing an AfterMiddleware for your iron server.

extern crate iron;
extern crate libflate;
extern crate brotli;

use std::io;
use std::io::Write;
use iron::prelude::*;
use iron::headers::*;
use iron::{AfterMiddleware};

use iron::headers::Encoding;
use iron::response::WriteBody;

const DEFAULT_MIN_BYTES_FOR_COMPRESSION: u64 = 860;

#[derive(PartialEq, Clone, Debug)]
enum CompressionEncoding {
    Brotli,
    Deflate,
    Gzip,
}

struct BrotliBody(Box<WriteBody>);

impl WriteBody for BrotliBody {
    fn write_body(&mut self, w: &mut Write) -> io::Result<()> {
        const BUFFER_SIZE: usize = 4096;
        const QUALITY: u32 = 8;
        const LG_WINDOW_SIZE: u32 = 20;
        let mut encoder = brotli::CompressorWriter::new(w, BUFFER_SIZE, QUALITY, LG_WINDOW_SIZE);
        self.0.write_body(&mut encoder)?;
        Ok(())
    }
}

struct GzipBody(Box<WriteBody>);

impl WriteBody for GzipBody {
    fn write_body(&mut self, w: &mut Write) -> io::Result<()> {
        let mut encoder = libflate::gzip::Encoder::new(w)?;
        self.0.write_body(&mut encoder)?;
        encoder.finish().into_result().map(|_| ())
    }
}

struct DeflateBody(Box<WriteBody>);

impl WriteBody for DeflateBody {
    fn write_body(&mut self, w: &mut Write) -> io::Result<()> {
        let mut encoder = libflate::deflate::Encoder::new(w);
        self.0.write_body(&mut encoder)?;
        encoder.finish().into_result().map(|_| ())
    }
}

fn encoding_matches_header(encoding: &CompressionEncoding, header: &Encoding) -> bool {
    match encoding {
        &CompressionEncoding::Brotli => *header == Encoding::EncodingExt(String::from("br")),
        &CompressionEncoding::Deflate => *header == Encoding::Deflate,
        &CompressionEncoding::Gzip => *header == Encoding::Gzip || *header == Encoding::EncodingExt(String::from("*")),
    }
}

fn get_body(encoding: &CompressionEncoding, wrapped_body: Box<WriteBody>) -> Box<WriteBody> {
    match encoding {
        &CompressionEncoding::Brotli => Box::new(BrotliBody(wrapped_body)),
        &CompressionEncoding::Deflate => Box::new(DeflateBody(wrapped_body)),
        &CompressionEncoding::Gzip => Box::new(GzipBody(wrapped_body)),
    }
}

fn get_header(encoding: &CompressionEncoding) -> Encoding {
    match encoding {
        &CompressionEncoding::Brotli => Encoding::EncodingExt(String::from("br")),
        &CompressionEncoding::Deflate => Encoding::Deflate,
        &CompressionEncoding::Gzip => Encoding::Gzip,
    }
}

fn which_compression<'a, 'b>(req: &'b Request, res: &'b Response, priority: &Vec<CompressionEncoding>) -> Option<CompressionEncoding> {
    return match (res.headers.get::<iron::headers::ContentEncoding>(), res.headers.get::<ContentLength>(), req.headers.get::<AcceptEncoding>()) {
        (None, Some(content_length), Some(&AcceptEncoding(ref quality_items))) => {
            if (content_length as &u64) < &DEFAULT_MIN_BYTES_FOR_COMPRESSION {
                return None;
            }

            let max_quality = quality_items.iter().map(|qi| qi.quality).max();

            if let Some(max_quality) = max_quality {
                let quality_items: Vec<&QualityItem<Encoding>> = quality_items
                    .iter()
                    .filter(|qi| qi.quality != Quality(0) && qi.quality == max_quality)
                    .collect();

                return priority
                    .iter()
                    .filter(|ce| quality_items.iter().find(|qi| {
                        encoding_matches_header(ce, &qi.item)
                    }).is_some())
                    .nth(0)
                    .map(|ce| ce.clone());
            }
            None
        }
        _ => None
    };
}

/// **Compression Middleware**
///
/// Currently either compresses using brotli, gzip or deflate algorithms. The algorithm is
/// chosen by evaluating the `AcceptEncoding` header sent by the client.
///
/// # Example
/// ```rust,no_run
/// extern crate iron;
/// extern crate iron_pack;
///
/// use iron::prelude::*;
/// use iron_pack::CompressionMiddleware;
///
/// fn a_lot_of_batman(_: &mut Request) -> IronResult<Response> {
///     let nana = "Na".repeat(5000);
///     Ok(Response::with((iron::status::Ok, format!("{}, Batman!", nana))))
/// }
///
/// fn main() {
///     let mut chain = Chain::new(a_lot_of_batman);
///     chain.link_after(CompressionMiddleware);
///     Iron::new(chain).http("localhost:3000").unwrap();
/// }
/// ```
pub struct CompressionMiddleware;

impl AfterMiddleware for CompressionMiddleware {

    /// Implementation of the compression middleware
    fn after(&self, req: &mut Request, mut res: Response) -> IronResult<Response> {
        let brotli = CompressionEncoding::Brotli;
        let deflate = CompressionEncoding::Deflate;
        let gzip = CompressionEncoding::Gzip;
        let default_priorities = vec!(brotli, gzip, deflate);

        if res.body.is_some() {
            if let Some(compression) = which_compression(&req, &res, &default_priorities) {
                res.headers.set(ContentEncoding(vec![get_header(&compression)]));
                res.headers.remove::<ContentLength>();
                res.body = Some(get_body(&compression, res.body.take().unwrap()));
            }
        }

        Ok(res)
    }
}

#[cfg(test)]
mod test_common {
    extern crate iron_test;

    use std::io::Read;
    use iron::prelude::*;
    use iron::headers::*;
    use iron::{Chain, status};
    use iron::modifiers::Header;
    use self::iron_test::{request};

    use super::CompressionMiddleware;

    pub fn build_compressed_echo_chain(with_encoding: bool) -> Chain {
        let mut chain = Chain::new(move |req: &mut Request| {
            let mut body: Vec<u8> = vec!();
            req.body.read_to_end(&mut body).unwrap();

            if !with_encoding {
                Ok(Response::with((status::Ok, body)))
            } else {
                Ok(Response::with((status::Ok, Header(ContentEncoding(vec![Encoding::Chunked])), body)))
            }
        });
        chain.link_after(CompressionMiddleware);
        return chain;
    }

    pub fn post_data_with_accept_encoding(data: &str, accept_encoding: Option<AcceptEncoding>, chain: &Chain) -> Response {
        let mut headers = Headers::new();
        if let Some(value) = accept_encoding {
            headers.set(value);
        }

        return request::post("http://localhost:3000/",
                             headers,
                             data,
                             chain).unwrap();
    }
}

#[cfg(test)]
mod uncompressable_tests {
    extern crate iron_test;

    use iron::headers::*;
    use self::iron_test::{response};

    use super::test_common::*;

    #[test]
    fn it_should_not_compress_response_when_client_does_not_send_accept_encoding_header() {
        let chain = build_compressed_echo_chain(false);
        let value = "a".repeat(1000);
        let res = post_data_with_accept_encoding(&value, None, &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), None);
        assert_eq!(response::extract_body_to_string(res), value);
    }

    #[test]
    fn it_should_not_compress_response_when_client_does_not_send_supported_encoding() {
        let chain = build_compressed_echo_chain(false);
        let value = "a".repeat(1000);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![qitem(Encoding::Chunked)])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), None);
        assert_eq!(response::extract_body_to_bytes(res), value.into_bytes());
    }

    #[test]
    fn it_should_not_compress_small_response() {
        let value = "a".repeat(10);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![qitem(Encoding::Gzip)])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), None);
        assert_eq!(response::extract_body_to_bytes(res), value.into_bytes());
    }

    #[test]
    fn it_should_not_compress_already_encoded_response() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(true);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![qitem(Encoding::Gzip)])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Chunked])));
        assert_eq!(response::extract_body_to_bytes(res), value.into_bytes());
    }
}

#[cfg(test)]
mod gzip_tests {
    extern crate iron_test;

    use std::io::Read;
    use iron::headers::*;
    use self::iron_test::{response};
    use libflate::gzip;

    use super::test_common::*;

    #[test]
    fn it_should_compress_response_body_correctly_using_gzip_and_set_header() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![qitem(Encoding::Gzip)])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentLength>(), None);
        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Gzip])));

        let compressed_bytes = response::extract_body_to_bytes(res);
        let mut decoder = gzip::Decoder::new(&compressed_bytes[..]).unwrap();
        let mut decoded_data = Vec::new();
        decoder.read_to_end(&mut decoded_data).unwrap();
        assert_eq!(decoded_data, value.into_bytes());
    }
}

#[cfg(test)]
mod deflate_tests {
    extern crate iron_test;

    use std::io::Read;
    use iron::headers::*;
    use self::iron_test::{response};
    use libflate::deflate;

    use super::test_common::*;

    #[test]
    fn it_should_compress_response_body_correctly_using_deflate_and_set_header() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![qitem(Encoding::Deflate)])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentLength>(), None);
        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Deflate])));

        let compressed_bytes = response::extract_body_to_bytes(res);
        let mut decoder = deflate::Decoder::new(&compressed_bytes[..]);
        let mut decoded_data = Vec::new();
        decoder.read_to_end(&mut decoded_data).unwrap();
        assert_eq!(decoded_data, value.into_bytes());
    }
}

#[cfg(test)]
mod brotli_tests {
    extern crate iron_test;

    use std::io::Read;
    use iron::headers::*;
    use self::iron_test::{response};
    use brotli;

    use super::test_common::*;

    #[test]
    fn it_should_compress_response_body_correctly_using_brotli_and_set_header() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![
                                                     qitem(Encoding::EncodingExt(String::from("br")))
                                                 ])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentLength>(), None);
        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::EncodingExt(String::from("br"))])));

        let compressed_bytes = response::extract_body_to_bytes(res);
        let mut decoder = brotli::Decompressor::new(&compressed_bytes[..], 4096);
        let mut decoded_data = Vec::new();
        decoder.read_to_end(&mut decoded_data).unwrap();
        assert_eq!(decoded_data, value.into_bytes());
    }
}

#[cfg(test)]
mod priority_tests {
    use iron::headers::*;

    use super::test_common::*;

    #[test]
    fn it_should_use_the_more_prior_compression_based_on_quality_for_gzip() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![
                                                     QualityItem { item: Encoding::Gzip, quality: q(0.5) },
                                                     QualityItem { item: Encoding::Deflate, quality: q(1.0) }
                                                 ])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Deflate])));
    }

    #[test]
    fn it_should_use_the_more_prior_compression_based_on_quality_for_deflate() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![
                                                     QualityItem { item: Encoding::Deflate, quality: q(1.0) },
                                                     QualityItem { item: Encoding::Gzip, quality: q(0.5) }
                                                 ])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Deflate])));
    }

    #[test]
    fn it_should_not_use_a_compression_with_quality_0() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![
                                                     QualityItem { item: Encoding::Gzip, quality: q(0.0) }
                                                 ])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), None);
    }

    #[test]
    fn it_should_use_the_brotli_compression_preferably_when_explicitly_sent() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![
                                                     qitem(Encoding::EncodingExt(String::from("*"))),
                                                     qitem(Encoding::Gzip),
                                                     qitem(Encoding::EncodingExt(String::from("br"))),
                                                     qitem(Encoding::Deflate),
                                                 ])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::EncodingExt(String::from("br"))])));
    }

    #[test]
    fn it_should_use_the_gzip_compression_if_the_any_encoding_is_sent() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![
                                                     qitem(Encoding::EncodingExt(String::from("*"))),
                                                     qitem(Encoding::Deflate),
                                                 ])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Gzip])));
    }

    #[test]
    fn it_should_use_the_gzip_compression_as_second_preference() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![
                                                     qitem(Encoding::Deflate),
                                                     qitem(Encoding::Gzip),
                                                 ])),
                                                 &chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Gzip])));
    }
}

#[cfg(all(feature = "unstable", test))]
mod middleware_benchmarks {
    macro_rules! bench_chain_with_header_and_size {
        ($name:ident, $chain:expr, $header:expr, $response_size:expr) => {
            #[bench]
            fn $name(b: &mut Bencher) {
                let chain = $chain;
                let mut rng = rand::IsaacRng::new_unseeded();

                b.iter(|| {
                    let data: String = rng.gen_ascii_chars().take($response_size).collect();
                    let _ = post_data_with_accept_encoding(&data,
                                                           $header,
                                                           &chain);
                })
            }
        };
    }

    macro_rules! bench_chains_with_size {
        ($mod_name:ident, $size:expr) => {
            mod $mod_name {
                extern crate test;
                extern crate rand;

                use std::io::Read;
                use iron::prelude::*;
                use iron::{Chain, status};
                use iron::headers::*;
                use self::test::Bencher;
                use self::rand::Rng;
                use super::super::test_common::*;

                fn build_echo_chain() -> Chain {
                    let chain = Chain::new(|req: &mut Request| {
                        let mut body: Vec<u8> = vec!();
                        req.body.read_to_end(&mut body).unwrap();
                        Ok(Response::with((status::Ok, body)))
                    });
                    return chain;
                }

                bench_chain_with_header_and_size!(without_middleware, build_echo_chain(), None, $size);
                bench_chain_with_header_and_size!(with_middleware_no_accept_header, build_compressed_echo_chain(false), None, $size);
                bench_chain_with_header_and_size!(with_middleware_gzip,
                                                  build_compressed_echo_chain(false),
                                                  Some(AcceptEncoding(vec![qitem(Encoding::Gzip)])),
                                                  $size);
                bench_chain_with_header_and_size!(with_middleware_deflate,
                                                  build_compressed_echo_chain(false),
                                                  Some(AcceptEncoding(vec![qitem(Encoding::Deflate)])),
                                                  $size);
                bench_chain_with_header_and_size!(with_middleware_brotli,
                                                  build_compressed_echo_chain(false),
                                                  Some(AcceptEncoding(vec![qitem(Encoding::EncodingExt(String::from("br")))])),
                                                  $size);
            }
        };
    }

    bench_chains_with_size!(response_1kb, 1024);
    bench_chains_with_size!(response_128kb, 128 * 1024);
    bench_chains_with_size!(response_1mb, 1024 * 1024);
}