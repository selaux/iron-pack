use std::io;
use iron::prelude::*;
use iron::headers::*;
use libflate::deflate;
use writer::ContentEncoding;

fn stringify_err(err: io::Error) -> String { format!("Error compressing body: {}", err) }

pub struct Deflate;

impl ContentEncoding for Deflate {
    fn get_header(&self) -> Encoding {
        Encoding::Deflate
    }

    fn compress_body(&self, res: &mut Response) -> Result<Vec<u8>, String> {
        if let Some(ref mut body) = res.body {
            let mut encoder = deflate::Encoder::new(Vec::new());
            body.write_body(&mut encoder).map_err(stringify_err)?;
            return encoder.finish().into_result().map_err(stringify_err);
        } else {
            Err(String::from("Error compressing body: No response body present."))
        }
    }
}