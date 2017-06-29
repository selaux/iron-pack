//! Compression middleware for Iron. This crate lets you automatically compress iron responses
//! by providing an AfterMiddleware for your iron server.

extern crate iron;
extern crate libflate;
extern crate brotli;

use iron::prelude::*;
use iron::headers::*;
use iron::{AfterMiddleware};

mod gzip;
mod deflate;
mod br;
mod compression_modifier;

pub use iron::headers::Encoding;
pub use iron::response::WriteBody;
pub use compression_modifier::CompressionModifier;
pub use gzip::GZipModifier;
pub use br::BrotliModifier;
pub use deflate::DeflateModifier;

const DEFAULT_MIN_BYTES_FOR_COMPRESSION: u64 = 860;

fn which_compression<'a, 'b>(req: &'b Request, res: &'b Response, priority: Vec<&'a CompressionModifier>) -> Option<&'a CompressionModifier> {
    return match (res.headers.get::<iron::headers::ContentEncoding>(), res.headers.get::<ContentLength>(), req.headers.get::<AcceptEncoding>()) {
        (None, Some(content_length), Some(&AcceptEncoding(ref quality_items))) => {
            if (content_length as &u64) < &DEFAULT_MIN_BYTES_FOR_COMPRESSION {
                return None;
            }

            let max_quality = quality_items.iter().map(|qi| qi.quality).max();
            let any_exists = quality_items.iter().find(|qi| qi.item == Encoding::EncodingExt(String::from("*"))).is_some();

            if let Some(max_quality) = max_quality {
                return quality_items
                    .iter()
                    .filter(|qi| qi.quality != Quality(0) && qi.quality == max_quality)
                    .filter_map(|qi: &'b QualityItem<Encoding>| priority.iter().find(|ce| {
                        let header = ce.get_header();
                        qi.item == header || header == Encoding::Gzip && any_exists
                    }))
                    .map(|ce: & &'a CompressionModifier| *ce)
                    .min_by_key(|ce1: & &'a CompressionModifier| priority.iter().position(|ce2: & &'a CompressionModifier| ce1.get_header() == ce2.get_header()));
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
        let brotli = br::BrotliModifier {};
        let gzip = gzip::GZipModifier {};
        let deflate = deflate::DeflateModifier {};
        let default_priorities: Vec<&CompressionModifier> = vec![
            &brotli,
            &gzip,
            &deflate
        ];

        if let Some(compression_modifier) = which_compression(&req, &res, default_priorities) {
            res.set_mut(compression_modifier);
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

    pub fn post_data_with_accept_encoding(data: &str, accept_encoding: Option<AcceptEncoding>, chain: Chain) -> Response {
        let mut headers = Headers::new();
        if let Some(value) = accept_encoding {
            headers.set(value);
        }

        return request::post("http://localhost:3000/",
                             headers,
                             data,
                             &chain).unwrap();
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
        let res = post_data_with_accept_encoding(&value, None, chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), None);
        assert_eq!(response::extract_body_to_string(res), value);
    }

    #[test]
    fn it_should_not_compress_response_when_client_does_not_send_supported_encoding() {
        let chain = build_compressed_echo_chain(false);
        let value = "a".repeat(1000);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![qitem(Encoding::Chunked)])),
                                                 chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), None);
        assert_eq!(response::extract_body_to_bytes(res), value.into_bytes());
    }

    #[test]
    fn it_should_not_compress_small_response() {
        let value = "a".repeat(10);
        let chain = build_compressed_echo_chain(false);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![qitem(Encoding::Gzip)])),
                                                 chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), None);
        assert_eq!(response::extract_body_to_bytes(res), value.into_bytes());
    }

    #[test]
    fn it_should_not_compress_already_encoded_response() {
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(true);
        let res = post_data_with_accept_encoding(&value,
                                                 Some(AcceptEncoding(vec![qitem(Encoding::Gzip)])),
                                                 chain);

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
                                                 chain);

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
                                                 chain);

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
                                                 chain);

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
                                                 chain);

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
                                                 chain);

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
                                                 chain);

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
                                                 chain);

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
                                                 chain);

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
                                                 chain);

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Gzip])));
    }
}
