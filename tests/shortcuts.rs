use mini_deck::shortcut::{KeyEvent, Shortcut};
#[test]
fn parsing_and_order() {
    let shortcut = Shortcut::parse(" shift + alt + CTRL + win + f12 ").unwrap();
    assert_eq!(shortcut.keys, vec![0x11, 0x12, 0x10, 0x5b, 0x7b]);
    for value in [
        "A", "0", "F1", "Enter", "Esc", "Tab", "Space", "Left", "Right", "Up", "Down",
    ] {
        assert!(Shortcut::parse(value).is_ok(), "{value}");
    }
}
#[test]
fn invalid_shortcuts() {
    for value in [
        "",
        "Ctrl",
        "Ctrl++A",
        "Ctrl+Ctrl+A",
        "A+B",
        "F0",
        "F13",
        "Ctrl+Unknown",
        "Ctrl+命令",
    ] {
        assert!(Shortcut::parse(value).is_err(), "{value}");
    }
}
#[test]
fn win32_event_sequence() {
    assert_eq!(
        Shortcut::parse("Ctrl+V").unwrap().events(),
        vec![
            KeyEvent {
                key: 0x11,
                up: false,
                extended: false
            },
            KeyEvent {
                key: 0x56,
                up: false,
                extended: false
            },
            KeyEvent {
                key: 0x56,
                up: true,
                extended: false
            },
            KeyEvent {
                key: 0x11,
                up: true,
                extended: false
            }
        ]
    );
    let s = Shortcut::parse("Win+Left").unwrap();
    assert!(s.events().iter().all(|e| e.extended));
    assert_eq!(
        s.releases().iter().map(|e| e.key).collect::<Vec<_>>(),
        vec![0x25, 0x5b]
    );
}
#[test]
fn partial_injection_cleanup_and_cleanup_failure() {
    use mini_deck::keyboard::submit_sequence;
    let shortcut = Shortcut::parse("Ctrl+Shift+Esc").unwrap();
    let mut calls = Vec::new();
    let result = submit_sequence(&shortcut, |events| {
        calls.push(events.to_vec());
        1
    });
    assert!(result.is_err());
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1], shortcut.releases());
    assert!(calls[1].iter().all(|e| e.up));
    let mut count = 0;
    assert!(submit_sequence(&shortcut, |events| {
        count += 1;
        events.len()
    })
    .is_ok());
    assert_eq!(count, 1);
}

#[test]
fn navigation_and_editing_keys_have_correct_virtual_keys_and_flags() {
    for (name, key, extended) in [
        ("PageUp", 0x21, true),
        ("PageDown", 0x22, true),
        ("Home", 0x24, true),
        ("End", 0x23, true),
        ("Insert", 0x2d, true),
        ("Delete", 0x2e, true),
        ("Backspace", 0x08, false),
    ] {
        let shortcut = Shortcut::parse(&format!("Ctrl+Shift+{name}")).unwrap();
        assert_eq!(shortcut.keys, vec![0x11, 0x10, key]);
        let events = shortcut.events();
        assert_eq!(
            (events[2].key, events[2].up, events[2].extended),
            (key, false, extended)
        );
        assert_eq!(
            (events[3].key, events[3].up, events[3].extended),
            (key, true, extended)
        );
        assert_eq!(
            shortcut
                .releases()
                .iter()
                .map(|e| e.key)
                .collect::<Vec<_>>(),
            vec![key, 0x10, 0x11]
        );
        assert_eq!(
            Shortcut::parse(&name.to_ascii_lowercase()).unwrap().keys,
            vec![key]
        );
    }
    for (alias, canonical) in [
        ("PgUp", "PageUp"),
        ("PgDn", "PageDown"),
        ("PgDown", "PageDown"),
        ("Ins", "Insert"),
        ("Del", "Delete"),
    ] {
        assert_eq!(
            Shortcut::parse(alias).unwrap(),
            Shortcut::parse(canonical).unwrap()
        );
    }
}
