use std::{cell::RefCell, collections::HashMap, io};

use super::with_doc;
use crate::ipc::{Pack, Unpack};

/// The most a slide's layers may take in memory.
const LAYER_MEMORY: usize = 512 * 1024 * 1024;

// Rendered layers wait here; each leaves on its own request.
thread_local! {
    static WAITING: RefCell<HashMap<u64, Vec<Option<Vec<u8>>>>> = RefCell::new(HashMap::new());
}

/// Slide `part`'s layers, its pixels held back.
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
        let ((width, height), layers) = doc.slide_layers((part, hash), (width, height), LAYER_MEMORY)?;
        let mut body = Pack::new()
            .u64(u64::from(width))
            .u64(u64::from(height))
            .u64(layers.len() as u64);
        let mut pixels = Vec::with_capacity(layers.len());
        for layer in layers {
            body = body.str(&layer.json).u64(u64::from(layer.pixels.is_some()));
            pixels.push(layer.pixels);
        }
        WAITING.with(|waiting| waiting.borrow_mut().insert(id, pixels));
        Ok(body.bytes())
    })?
}

/// Hands over layer `index`'s pixels once, then frees them.
pub(super) fn slide_layer(unpack: &mut Unpack) -> io::Result<Vec<u8>> {
    let (id, index) = (unpack.u64()?, unpack.u64()?);
    let taken = WAITING.with(|waiting| {
        let mut waiting = waiting.borrow_mut();
        let layers = waiting.get_mut(&id)?;
        let pixels = layers.get_mut(usize::try_from(index).ok()?)?.take();
        if layers.iter().all(Option::is_none) {
            waiting.remove(&id);
        }
        pixels
    });
    let pixels = taken.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("no layer {index} waits")))?;
    Ok(Pack::new().blob(&pixels).bytes())
}
