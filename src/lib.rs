extern crate iron;
extern crate libflate;

use iron::prelude::*;
use iron::headers::*;
use iron::{AfterMiddleware};

mod gzip_writer;

const MIN_COMPRESSABLE_SIZE: u64 = 860;

fn which_compression(req: &Request, res: &Response) -> Option<Encoding> {
    {
        let encoding = res.headers.get::<ContentEncoding>();
        if encoding != None {
            return None;
        }
    }

    {
        let length = res.headers.get::<ContentLength>().unwrap();
        if (length as &u64) < &MIN_COMPRESSABLE_SIZE {
            return None;
        }
    }

    {
        if let Some(&AcceptEncoding(ref quality_items)) = req.headers.get::<AcceptEncoding>() {
            let allowed_qi = quality_items.iter().find(|qi| qi.item == Encoding::Gzip);

            if let Some(&QualityItem { item: ref encoding, quality: _ }) = allowed_qi {
                return Some(encoding.clone());
            }
        }
    }

    return None;
}

pub struct CompressionMiddleware;

impl AfterMiddleware for CompressionMiddleware {
    fn after(&self, req: &mut Request, mut res: Response) -> IronResult<Response> {
        let compression = which_compression(&req, &res);

        match compression {
            Some(Encoding::Gzip) => {
                res.set_mut(gzip_writer::GzipWriter);
                Ok(res)
            },
            _ => Ok(res)
        }
    }
}

#[cfg(test)]
mod gzip_tests {
    extern crate iron_test;

    use std::io::Read;
    use iron::prelude::*;
    use iron::headers::*;
    use iron::{Chain, Headers, status};
    use iron::modifiers::Header;
    use self::iron_test::{request, response};
    use libflate::gzip;

    use super::CompressionMiddleware;

    fn build_compressed_echo_chain(with_encoding: bool) -> Chain {
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
