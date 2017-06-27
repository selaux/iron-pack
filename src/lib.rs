//! Compression middleware for Iron. This crate lets you automatically compress iron responses
//! by providing an AfterMiddleware for your iron server.

extern crate iron;
extern crate libflate;
extern crate brotli;

use std::cmp::Ordering::{Equal, Greater, Less};
use iron::prelude::*;
use iron::headers::*;
use iron::{AfterMiddleware};

mod gzip_writer;
mod deflate_writer;
mod brotli_writer;

const DEFAULT_MIN_BYTES_FOR_COMPRESSION: u64 = 860;

fn which_compression(req: &Request, res: &Response) -> Option<Encoding> {
    {
        let encoding = res.headers.get::<ContentEncoding>();
        if encoding != None {
            return None;
        }
    }

    {
        if let Some(length) = res.headers.get::<ContentLength>() {
            if (length as &u64) < &DEFAULT_MIN_BYTES_FOR_COMPRESSION {
                return None;
            }
        }
    }

    {
        let brotli_encoding = Encoding::EncodingExt(String::from("br"));
        if let Some(&AcceptEncoding(ref quality_items)) = req.headers.get::<AcceptEncoding>() {
            let max_quality = quality_items.iter().map(|qi| qi.quality).max();

            if let Some(max_quality) = max_quality {
                let internal_priority = vec![
                    brotli_encoding,
                    Encoding::Gzip,
                    Encoding::EncodingExt(String::from("*")),
                    Encoding::Deflate
                ];
                let allowed_qi = quality_items
                    .iter()
                    .filter(|qi| qi.quality == max_quality)
                    .filter(|qi| internal_priority.iter().position(|q| q == &qi.item) != None)
                    .min_by(|qi1, qi2| {
                        match (internal_priority.iter().position(|qi| qi == &qi1.item), internal_priority.iter().position(|qi| qi == &qi2.item)) {
                            (Some(a), Some(b)) => {
                                if a > b {
                                    return Greater;
                                };
                                if a < b {
                                    return Less;
                                }
                                Equal
                            },
                            _ => Equal
                        }
                    })
                    .map(|encoding| match &encoding.item {
                        &Encoding::EncodingExt(ref s) => {
                            if s == "*" {
                                return Encoding::Gzip;
                            }
                            encoding.item.clone()
                        },
                        e => e.clone()
                    });

                if let Some(encoding) = allowed_qi {
                    return Some(encoding);
                }
            }
        }
    }

    return None;
}

/// **Compression Middleware**
///
/// Currently either compresses using gzip or deflate algorithms. The algorithm is
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
        let compression = which_compression(&req, &res);

        match compression {
            Some(Encoding::Gzip) => {
                res.set_mut(gzip_writer::GzipWriter);
                Ok(res)
            },
            Some(Encoding::Deflate) => {
                res.set_mut(deflate_writer::DeflateWriter);
                Ok(res)
            },
            Some(Encoding::EncodingExt(s)) => {
                if s == "br" {
                    res.set_mut(brotli_writer::BrotliWriter);
                }
                Ok(res)
            },
            _ => Ok(res)
        }
    }
}

#[cfg(test)]
mod test_common {
    use std::io::Read;
    use iron::prelude::*;
    use iron::headers::*;
    use iron::{Chain, status};
    use iron::modifiers::Header;

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
}

#[cfg(test)]
mod uncompressable_tests {
    extern crate iron_test;

    use iron::headers::*;
    use iron::Headers;
    use self::iron_test::{request, response};

    use super::test_common::*;

    #[test]
    fn it_should_not_compress_when_client_does_not_accept() {
        let chain = build_compressed_echo_chain(false);
        let value = "a".repeat(1000);
        let res = request::post("http://localhost:3000/",
                                Headers::new(),
                                &value,
                                &chain).unwrap();


        {
            assert_eq!(res.headers.get::<ContentEncoding>(), None);
        }
        assert_eq!(response::extract_body_to_bytes(res), value.into_bytes());
    }

    #[test]
    fn it_should_not_compress_when_client_does_not_accept_in_header() {
        let mut headers = Headers::new();
        let chain = build_compressed_echo_chain(false);
        let value = "a".repeat(1000);

        headers.set(
            AcceptEncoding(vec![qitem(Encoding::Chunked)])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();


        {
            assert_eq!(res.headers.get::<ContentEncoding>(), None);
        }
        assert_eq!(response::extract_body_to_bytes(res), value.into_bytes());
    }

    #[test]
    fn it_should_not_compress_tiny_responses() {
        let mut headers = Headers::new();
        let value = "a".repeat(10);
        let chain = build_compressed_echo_chain(false);

        headers.set(
            AcceptEncoding(vec![qitem(Encoding::Gzip)])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        {
            assert_eq!(res.headers.get::<ContentEncoding>(), None);
        }
        assert_eq!(response::extract_body_to_bytes(res), value.into_bytes());
    }

    #[test]
    fn it_should_not_compress_encoded_responses() {
        let mut headers = Headers::new();
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(true);

        headers.set(
            AcceptEncoding(vec![qitem(Encoding::Gzip)])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        {
            assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Chunked])));
        }
        assert_eq!(response::extract_body_to_bytes(res), value.into_bytes());
    }
}

#[cfg(test)]
mod gzip_tests {
    extern crate iron_test;

    use std::io::Read;
    use iron::headers::*;
    use iron::Headers;
    use self::iron_test::{request, response};
    use libflate::gzip;

    use super::test_common::*;

    #[test]
    fn it_should_compress_long_response() {
        let mut headers = Headers::new();
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);

        headers.set(
            AcceptEncoding(vec![qitem(Encoding::Gzip)])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        {
            assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Gzip])));
        }

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
    use iron::Headers;
    use self::iron_test::{request, response};
    use libflate::deflate;

    use super::test_common::*;

    #[test]
    fn it_should_compress_long_response() {
        let mut headers = Headers::new();
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);

        headers.set(
            AcceptEncoding(vec![qitem(Encoding::Deflate)])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        {
            assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Deflate])));
        }

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
    use iron::Headers;
    use self::iron_test::{request, response};
    use brotli;

    use super::test_common::*;

    #[test]
    fn it_should_compress_long_response() {
        let mut headers = Headers::new();
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);

        headers.set(
            AcceptEncoding(vec![qitem(Encoding::EncodingExt(String::from("br")))])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        {
            assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::EncodingExt(String::from("br"))])));
        }

        let compressed_bytes = response::extract_body_to_bytes(res);
        let mut decoder = brotli::Decompressor::new(&compressed_bytes[..], 4096);
        let mut decoded_data = Vec::new();
        decoder.read_to_end(&mut decoded_data).unwrap();
        assert_eq!(decoded_data, value.into_bytes());
    }
}

#[cfg(test)]
mod priority_tests {
    extern crate iron_test;

    use iron::headers::*;
    use iron::Headers;
    use self::iron_test::request;

    use super::test_common::*;

    #[test]
    fn it_should_use_the_more_prior_algorithm() {
        let mut headers = Headers::new();
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);

        headers.set(
            AcceptEncoding(vec![
                QualityItem { item: Encoding::Gzip, quality: q(0.5) },
                QualityItem { item: Encoding::Deflate, quality: q(1.0) }
            ])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Deflate])));
    }

    #[test]
    fn it_should_use_the_more_prior_algorithm_2() {
        let mut headers = Headers::new();
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);

        headers.set(
            AcceptEncoding(vec![
                QualityItem { item: Encoding::Deflate, quality: q(1.0) },
                QualityItem { item: Encoding::Gzip, quality: q(0.5) }
            ])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Deflate])));
    }

    #[test]
    fn it_should_use_the_brotli_algorithm_preferably_when_supported() {
        let mut headers = Headers::new();
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);

        headers.set(
            AcceptEncoding(vec![
                qitem(Encoding::EncodingExt(String::from("*"))),
                qitem(Encoding::Gzip),
                qitem(Encoding::EncodingExt(String::from("br"))),
                qitem(Encoding::Deflate),
            ])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::EncodingExt(String::from("br"))])));
    }

    #[test]
    fn it_should_use_the_gzip_algorithm_for_the_any_encoding() {
        let mut headers = Headers::new();
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);

        headers.set(
            AcceptEncoding(vec![
                qitem(Encoding::EncodingExt(String::from("*"))),
                qitem(Encoding::Deflate),
            ])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Gzip])));
    }

    #[test]
    fn it_should_use_the_gzip_algorithm_as_second_preference() {
        let mut headers = Headers::new();
        let value = "a".repeat(1000);
        let chain = build_compressed_echo_chain(false);

        headers.set(
            AcceptEncoding(vec![
                qitem(Encoding::Deflate),
                qitem(Encoding::Gzip),
            ])
        );
        let res = request::post("http://localhost:3000/",
                                headers,
                                &value,
                                &chain).unwrap();

        assert_eq!(res.headers.get::<ContentEncoding>(), Some(&ContentEncoding(vec![Encoding::Gzip])));
    }
}
