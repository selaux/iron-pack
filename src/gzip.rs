use std::io;
use iron::prelude::*;
use iron::headers::*;
use libflate::gzip;
use writer::ContentEncoding;

fn stringify_err(err: io::Error) -> String { format!("Error compressing body: {}", err) }

pub struct GZip;

impl ContentEncoding for GZip {
    fn get_header(&self) -> Encoding {
        Encoding::Gzip
    }

    fn compress_body(&self, res: &mut Response) -> Result<Vec<u8>, String> {
        if let Some(ref mut body) = res.body {
            let mut encoder = gzip::Encoder::new(Vec::new()).map_err(stringify_err)?;
            body.write_body(&mut encoder).map_err(stringify_err)?;
            return encoder.finish().into_result().map_err(stringify_err);
        } else {
            Err(String::from("Error compressing body: No response body present."))
        }
    }
}