use std::path::PathBuf;

use crate::linker::{LinkThrough, Module};

fn file_name_to_module(name: String, parent: PathBuf) -> Module {
    Module {
        name: name.clone(),
        decl_path: parent.join(name + ".rs"),
        children: Vec::with_capacity(0),
        link_privacy: None,
    }
}

pub struct ModuleBuilder {
    pub(super) path: PathBuf,
    name: String,
    modules: Vec<SubModule>,
    files: Vec<String>,
}

impl ModuleBuilder {
    pub fn new(root: PathBuf, name: String) -> Self {
        assert!(
            root.ends_with("lib.rs") || root.ends_with("main.rs"),
            "Root is not a valid rust file (main.rs | lib.rs)"
        );

        Self {
            name,
            path: root,
            modules: Vec::new(),
            files: Vec::new(),
        }
    }

    pub fn build(self) -> Module {
        self.build_children().link().into_module()
    }

    fn build_children(mut self) -> Self {
        let Ok(dir) = self.path.parent().unwrap().read_dir() else {
            return self;
        };

        for entry_res in dir {
            let Ok(entry) = entry_res else {
                continue;
            };

            let path = entry.path();

            let file_kind = entry.file_type().unwrap();

            if file_kind.is_dir() {
                let mut new = SubModule::new(path);
                new.build_children();
                self.modules.push(new)
            } else if file_kind.is_file() && path.extension() == Some("rs".as_ref()) {
                let name = path.file_name().unwrap().to_string_lossy();
                match name {
                    n if n == "lib.rs" || n == "main.rs" => {}
                    n => self.files.push(n.trim_end_matches(".rs").to_owned()),
                }
            }
        }
        self
    }

    fn link(mut self) -> Self {
        for c in &mut self.modules {
            c.link();

            if self.files.contains(&c.name) {
                c.link.update_link(LinkThrough::Name);
            }
        }
        self
    }

    fn into_module(self) -> Module {
        let parent = self.path.parent().unwrap();
        Module {
            children: self
                .files
                .into_iter()
                .filter_map(|name| {
                    if &name == "lib.rs" || &name == "main.rs" {
                        return None;
                    }
                    Some(file_name_to_module(name, parent.to_path_buf()))
                })
                .chain(self.modules.into_iter().map(|m| m.into_module()))
                .collect(),
            decl_path: self.path,
            name: self.name,
            link_privacy: None,
        }
    }
}

#[derive(PartialEq, Debug)]
struct SubModule {
    path: PathBuf,
    name: String,
    link: LinkThrough,
    modules: Vec<SubModule>,
    files: Vec<String>,
}

impl SubModule {
    fn new(root: PathBuf) -> Self {
        Self {
            name: root.file_name().unwrap().to_string_lossy().into_owned(),
            path: root,
            link: LinkThrough::None,
            modules: Vec::new(),
            files: Vec::new(),
        }
    }

    fn build_children(&mut self) {
        let Ok(dir) = self.path.read_dir() else {
            return;
        };

        for entry_res in dir {
            let Ok(entry) = entry_res else {
                continue;
            };

            let path = entry.path();

            let file_kind = entry.file_type().unwrap();

            if file_kind.is_dir() {
                let mut new = SubModule::new(path);
                new.build_children();
                self.modules.push(new)
            } else if file_kind.is_file() && path.extension() == Some("rs".as_ref()) {
                self.files.push(
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .trim_end_matches(".rs")
                        .to_owned(),
                );
            }
        }
    }

    fn link(&mut self) {
        for c in &mut self.modules {
            c.link();

            if self.files.contains(&c.name) {
                c.link.update_link(LinkThrough::Name);
            }
        }

        if self.files.iter().any(|f| f == "mod.rs") {
            self.link = LinkThrough::ModChild;
        }
    }

    fn into_module(self) -> Module {
        Module {
            children: self
                .files
                .into_iter()
                .filter_map(|name| {
                    if &name == "mod.rs" {
                        return None;
                    }
                    Some(file_name_to_module(name, self.path.clone()))
                })
                .chain(self.modules.into_iter().map(|m| m.into_module()))
                .collect(),
            decl_path: match self.link {
                LinkThrough::Name => self
                    .path
                    .parent()
                    .unwrap()
                    .to_path_buf()
                    .join(self.name.clone() + ".rs"),
                LinkThrough::ModChild => self.path.join("mod.rs"),
                LinkThrough::None => self.path.join("mod.rs"),
            },
            name: self.name,
            link_privacy: None,
        }
    }
}
