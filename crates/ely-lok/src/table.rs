//! The cursor's table borders, as Writer reports them (DOC-07).

use std::io;

use serde_json::Value;

use crate::ipc::{Pack, Unpack};

/// One axis in twips: columns run across, rows down.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lines {
    /// The first line, from `offset`.
    pub left: i32,
    /// The last line, from `offset`.
    pub right: i32,
    pub offset: i32,
    pub inner: Vec<Line>,
}

/// An inner line and the kernel's bounds for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub position: i32,
    pub min: i32,
    pub max: i32,
    pub hidden: bool,
}

/// The table holding the cursor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub columns: Lines,
    pub rows: Lines,
}

/// Reads a `LOK_CALLBACK_TABLE_SELECTED` payload; `{}` is no table.
pub fn read(text: &str) -> Result<Option<Table>, String> {
    let fault = || format!("an unreadable table: {text}");
    let json: Value = serde_json::from_str(text).map_err(|_| fault())?;
    let object = json.as_object().ok_or_else(fault)?;
    if object.is_empty() {
        return Ok(None);
    }
    let int = |value: Option<&Value>| i32::try_from(value?.as_i64()?).ok();
    let lines = |key: &str| -> Option<Lines> {
        let axis = object.get(key)?;
        let inner = axis.get("entries")?.as_array()?.iter().map(|entry| {
            Some(Line {
                position: int(entry.get("position"))?,
                min: int(entry.get("min"))?,
                max: int(entry.get("max"))?,
                hidden: entry.get("hidden")?.as_bool()?,
            })
        });
        Some(Lines {
            left: int(axis.get("left"))?,
            right: int(axis.get("right"))?,
            offset: int(axis.get("tableOffset"))?,
            inner: inner.collect::<Option<_>>()?,
        })
    };
    let columns = lines("columns").ok_or_else(fault)?;
    let rows = lines("rows").ok_or_else(fault)?;
    Ok(Some(Table { columns, rows }))
}

impl Table {
    pub(crate) fn pack(table: Option<&Table>, pack: Pack) -> Pack {
        let Some(table) = table else {
            return pack.u64(0);
        };
        [&table.columns, &table.rows].into_iter().fold(pack.u64(1), |pack, axis| {
            let pack = pack
                .i64(axis.left.into())
                .i64(axis.right.into())
                .i64(axis.offset.into())
                .u64(axis.inner.len() as u64);
            axis.inner.iter().fold(pack, |pack, line| {
                pack.i64(line.position.into())
                    .i64(line.min.into())
                    .i64(line.max.into())
                    .u64(line.hidden as u64)
            })
        })
    }

    pub(crate) fn unpack(unpack: &mut Unpack) -> io::Result<Option<Table>> {
        if unpack.u64()? == 0 {
            return Ok(None);
        }
        let mut lines = || -> io::Result<Lines> {
            let mut int = || i32::try_from(unpack.i64()?).map_err(io::Error::other);
            let (left, right, offset) = (int()?, int()?, int()?);
            let inner = (0..unpack.u64()?)
                .map(|_| {
                    let mut int = || i32::try_from(unpack.i64()?).map_err(io::Error::other);
                    let (position, min, max) = (int()?, int()?, int()?);
                    Ok(Line { position, min, max, hidden: unpack.u64()? != 0 })
                })
                .collect::<io::Result<_>>()?;
            Ok(Lines { left, right, offset, inner })
        };
        let columns = lines()?;
        let rows = lines()?;
        Ok(Some(Table { columns, rows }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEEN: &str = r#"{ "columns": { "left": 0, "right": 9638, "tableOffset": 1135, "entries": [ { "position": 4819, "min": 300, "max": 9338, "hidden": false } ] }, "rows": { "left": 0, "right": 552, "tableOffset": 1418, "entries": [ { "position": 276, "min": 100, "max": 552, "hidden": false } ] } }"#;

    #[test]
    fn a_table_reads_and_crosses_the_wire() {
        assert_eq!(read("{ }"), Ok(None), "no table");
        let table = read(SEEN).expect("readable").expect("a table");
        assert_eq!(table.columns.offset, 1135);
        assert_eq!(table.rows.inner[0].position, 276);
        let text = SEEN.replace("\"left\": 0,", "\"left\": \"0\",");
        assert!(read(&text).is_err(), "a number as text is a fault");
        assert!(read("[]").is_err());
        let bytes = Table::pack(Some(&table), Pack::new()).bytes();
        let back = Table::unpack(&mut Unpack::new(&bytes)).expect("unpacks");
        assert_eq!(back, Some(table));
        let none = Table::pack(None, Pack::new()).bytes();
        assert_eq!(Table::unpack(&mut Unpack::new(&none)).expect("unpacks"), None);
    }
}
