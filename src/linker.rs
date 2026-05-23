use core::fmt;
use std::{
    fmt::Display,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use regex::RegexBuilder;

use crate::{LinkKind, error_exit};

#[derive(PartialEq, Default, Debug)]
pub enum LinkThrough {
    /// Has a sibling in the parent folder with the same name as the module
    Name,
    /// Has a child in the folder with the name `mod`
    ModChild,
    /// Currently no linking method
    #[default]
    None,
}

impl LinkThrough {
    pub fn update_link(&mut self, kind: LinkThrough) {
        *self = match self {
            LinkThrough::None => kind,
            _ => panic!("Cannot have multiple files acting as the base for one module"),
        };
    }
}

#[derive(Debug)]
pub struct Module {
    /// The path to the file declaring the module.
    pub decl_path: PathBuf,
    /// The name of the module.
    pub name: String,
    pub(crate) link_privacy: Option<LinkKind>,
    pub children: Vec<Module>,
}

const MOD_FINDER_REGEX: &str = r"(?m)^\s*(?:(pub(?:\(crate\)|\(super\)|\(self\))?)\s+)?mod\s+(\w+)";

impl Module {
    pub fn update_childrens_linked_status(&mut self) {
        let file_inner = fs::read_to_string(&self.decl_path)
            .map_err(|e| error_exit(e))
            .unwrap();

        let regex = RegexBuilder::new(MOD_FINDER_REGEX).build().unwrap();

        for cap in regex.captures_iter(&file_inner) {
            let privacy = cap
                .get(1)
                .map(|m| LinkKind::from_rust_syntax(m.as_str()).unwrap())
                .unwrap_or(LinkKind::Private);

            let name = cap.get(2).unwrap().as_str();

            self.children
                .iter_mut()
                .find(|Module { name: m_name, .. }| m_name == name)
                .map(|m| m.link_privacy = Some(privacy));
        }

        drop(file_inner);

        for c in &mut self.children {
            c.update_childrens_linked_status();
        }
    }

    pub(crate) fn link_unlinked_children(&mut self, link_kind: LinkKind, dry: bool) {
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.decl_path)
            .unwrap();

        for c in &mut self.children {
            if c.link_privacy.is_none() {
                if !dry {
                    file.write_all(
                        (link_kind.to_rust_syntax(&c.name) + "mod " + &c.name + ";\n").as_bytes(),
                    )
                    .expect("Failed to write to file");
                }
                c.link_privacy = Some(link_kind)
            }

            c.link_unlinked_children(link_kind, dry);
        }
    }

    fn fmt_with_indent(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        // Print this module's name
        for _ in 0..indent {
            write!(f, "    ")?; // 4‑space indent
        }
        writeln!(
            f,
            "`{}` {}[{}]",
            self.name,
            " ".repeat(15usize.saturating_sub(self.name.len())),
            self.link_privacy
                .as_ref()
                .map_or("unlinked".to_string(), LinkKind::to_string),
        )?;

        // Print children
        for child in &self.children {
            child.fmt_with_indent(f, indent + 1)?;
        }

        Ok(())
    }
}

impl Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_indent(f, 0)
    }
}

#[cfg(test)]
mod tests {
    use regex::RegexBuilder;

    use crate::linker::MOD_FINDER_REGEX;

    #[test]
    fn test_mod_finder_regex() {
        let input = r#"
mod basic;
pub mod public_mod;
pub(crate) mod crate_mod;
pub(super) mod super_mod;
pub(self) mod self_mod;

// should not match
let x = "mod fake";
"#;

        let regex = RegexBuilder::new(MOD_FINDER_REGEX).build().unwrap();

        let matches: Vec<(Option<&str>, &str)> = regex
            .captures_iter(input)
            .map(|cap| (cap.get(1).map(|m| m.as_str()), cap.get(2).unwrap().as_str()))
            .collect();

        assert_eq!(
            matches,
            vec![
                (None, "basic"),
                (Some("pub"), "public_mod"),
                (Some("pub(crate)"), "crate_mod"),
                (Some("pub(super)"), "super_mod"),
                (Some("pub(self)"), "self_mod"),
            ]
        );
    }
}
