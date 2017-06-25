use std::io;
use iron::prelude::*;
use iron::headers::*;
use iron::modifier::Modifier;
use libflate::deflate;

fn stringify_err(err: io::Error) -> String { format!("Error compressing body: {}", err) }

pub struct DeflateWriter;

impl DeflateWriter {
    fn get_compressed_body(&self, res: &mut Response) -> Result<Vec<u8>, String> {
        if let Some(ref mut body) = res.body {
            let mut encoder = deflate::Encoder::new(Vec::new());
            body.write_body(&mut encoder).map_err(stringify_err)?;
            return encoder.finish().into_result().map_err(stringify_err);
        } else {
            Err(String::from("Error compressing body: No response body present."))
        }
    }
}

impl Modifier<Response> for DeflateWriter {
    fn modify(self, mut res: &mut Response) {
        let compressed = self.get_compressed_body(&mut res);

        if let Ok(compressed_bytes) = compressed {
            res.headers.set(ContentEncoding(vec![Encoding::Deflate]));
            compressed_bytes.modify(res);
        }
    }
}