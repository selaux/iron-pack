use iron::prelude::*;
use iron::headers::*;
use iron::modifier::Modifier;
use libflate::deflate;

pub struct DeflateWriter;

impl DeflateWriter {
    fn get_compressed_body(&self, res: &mut Response) -> Option<Vec<u8>> {
        if let Some(ref mut body) = res.body {
            let mut encoder = deflate::Encoder::new(Vec::new());
            body.write_body(&mut encoder).unwrap();
            return Some(encoder.finish().into_result().unwrap());
        } else {
            None
        }
    }
}

impl Modifier<Response> for DeflateWriter {
    fn modify(self, mut res: &mut Response) {
        let compressed = self.get_compressed_body(&mut res);

        if let Some(compressed_bytes) = compressed {
            res.headers.set(ContentEncoding(vec![Encoding::Deflate]));
            compressed_bytes.modify(res);
        }
    }
}