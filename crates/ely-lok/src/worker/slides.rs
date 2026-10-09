use std::io;

use super::with_doc;
use crate::ipc::{self, Pack, Unpack};

/// Slide `part`'s slideshow layers at about the asked pixels.
pub(super) fn slide_layers(unpack: &mut Unpack) -> io::Result<Vec<u8>> {
    let (id, part) = (unpack.u64()?, unpack.u64()?);
    let (width, height) = (unpack.u64()?, unpack.u64()?);
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidInput, what.to_string());
    let part = i32::try_from(part).map_err(|_| bad("bad part"))?;
    let pixels = (u32::try_from(width), u32::try_from(height));
    let (Ok(width), Ok(height)) = pixels else {
        return Err(bad("bad size"));
    };
    with_doc(id, |doc| {
        let info: serde_json::Value =
            serde_json::from_str(&doc.presentation_info()?).map_err(io::Error::other)?;
        let slide = usize::try_from(part).ok().and_then(|at| info["slides"].get(at));
        let hash = slide
            .and_then(|slide| slide["hash"].as_str())
            .ok_or_else(|| bad("no such slide"))?;
        let ((width, height), layers) =
            doc.slide_layers((part, hash), (width, height), ipc::MAX_FRAME - 1024)?;
        let mut body = Pack::new()
            .u64(u64::from(width))
            .u64(u64::from(height))
            .u64(layers.len() as u64);
        for layer in &layers {
            body = body.str(&layer.json).u64(u64::from(layer.pixels.is_some()));
            if let Some(pixels) = &layer.pixels {
                body = body.blob(pixels);
            }
        }
        Ok(body.bytes())
    })?
}
