use iron::prelude::*;
use iron::headers::*;
use iron::modifier::Modifier;

pub trait ContentEncoding {
    fn get_header(&self) -> Encoding;
    fn compress_body(&self, res: &mut Response) -> Result<Vec<u8>, String>;
}

impl PartialEq for ContentEncoding {
    fn eq(&self, other: &ContentEncoding) -> bool {
        self.get_header() == other.get_header()
    }

    fn ne(&self, other: &ContentEncoding) -> bool {
        self.get_header() != other.get_header()
    }
}

impl<'a> Modifier<Response> for &'a ContentEncoding {
    fn modify(self, mut res: &mut Response) {
        let compressed = self.compress_body(&mut res);

        if let Ok(compressed_bytes) = compressed {
            res.headers.set(ContentEncoding(vec![self.get_header()]));
            compressed_bytes.modify(res);
        }
    }
}