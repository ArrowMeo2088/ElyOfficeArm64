use super::*;

fn item(command: &str, enabled: bool) -> String {
    format!(r#"{{"text":"x","type":"command","command":"{command}","enabled":"{enabled}"}}"#)
}

#[test]
fn spelling_rows_come_first_class() {
    let menu = r#"{"menu":[{"text":"The","type":"command","command":".uno:SpellCheckApplySuggestion?ApplyRule:string=Spelling_The","enabled":"true"},{"type":"separator"},{"text":"Ignore All","type":"command","command":".uno:SpellCheckIgnoreAll?Type:string=Spelling","enabled":"true"}]}"#;
    let offered = read(menu).expect("the menu parses");
    let spelling = offered.spelling.expect("a spelling menu");
    assert_eq!(spelling.suggestions, [("The".into(), "Spelling_The".into())]);
    assert_eq!(offered.target, Target::Text);
    let other = format!(r#"{{"menu":[{}]}}"#, item(".uno:Cut", true));
    assert_eq!(read(&other).expect("the menu parses").spelling, None);
}

#[test]
fn the_menu_names_its_target_and_flags() {
    let table = format!(
        r#"{{"menu":[{},{{"text":"Insert","type":"menu","command":".uno:TableInsertMenu","menu":[{}],"enabled":"true"}}]}}"#,
        item(".uno:MergeCells", false),
        item(".uno:InsertRowsBefore", true),
    );
    let table = read(&table).expect("the table menu parses");
    assert_eq!(table.target, Target::Table);
    assert!(table.allows(".uno:InsertRowsBefore"), "submenu rows count");
    assert!(!table.allows(".uno:MergeCells"), "a disabled row");
    assert!(!table.allows(".uno:DeleteTable"), "an absent row");
    let frame = format!(
        r#"{{"menu":[{{"type":"menu","menu":[{}],"enabled":"true"}}]}}"#,
        item(".uno:WrapMenu", true)
    );
    assert_eq!(read(&frame).expect("a commandless submenu").target, Target::Frame);
    let text = format!(r#"{{"menu":[{}]}}"#, item(".uno:Paste", true));
    assert_eq!(read(&text).expect("parses").target, Target::Text);
}

#[test]
fn a_malformed_menu_fails() {
    assert!(read(r#"{"menu":[{"type":"command","command":".uno:Cut"}]}"#).is_err());
    assert!(read(r#"{"menu":[{"type":"command","enabled":"true"}]}"#).is_err());
    assert!(read(r#"{"items":[]}"#).is_err());
}
