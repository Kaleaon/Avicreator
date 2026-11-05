//! Built-in form programs shared by native, browser and WebAssembly callers.

pub const FORMS: &[(&str, &str)] = &[
    ("human", include_str!("../programs/human.json")),
    ("canine", include_str!("../programs/canine.json")),
    ("feline", include_str!("../programs/feline.json")),
    ("vulpine", include_str!("../programs/vulpine.json")),
    ("minotaur", include_str!("../programs/minotaur.json")),
    ("centaur", include_str!("../programs/centaur.json")),
    ("scaly", include_str!("../programs/scaly.json")),
    ("lizard", include_str!("../programs/lizard.json")),
    ("dragon", include_str!("../programs/dragon.json")),
];

pub fn get(name: &str) -> Option<&'static str> {
    FORMS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, text)| *text)
}
