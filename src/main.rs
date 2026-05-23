use std::fmt::{Debug, Display};
use std::path::PathBuf;
use std::process::exit;

use cargo_metadata::MetadataCommand;
use clap::Parser;

use crate::module_builder::ModuleBuilder;

mod linker;
pub mod module_builder;

#[derive(Clone, Copy, clap::ValueEnum, Debug)]
enum LinkKind {
    Private,
    Public,
    Crate,
    Super,
}

impl LinkKind {
    pub fn to_rust_syntax(&self, name: &str) -> String {
        let vis = match self {
            LinkKind::Private => "",
            LinkKind::Public => "pub ",
            LinkKind::Crate => "pub(crate) ",
            LinkKind::Super => "pub(super) ",
        };

        format!("{vis}mod {name}")
    }

    pub fn from_rust_syntax(syn: &str) -> Option<Self> {
        match syn {
            "pub(self)" => Some(LinkKind::Private),
            "pub(crate)" => Some(LinkKind::Crate),
            "pub(super)" => Some(LinkKind::Super),
            "pub" => Some(LinkKind::Public),
            _ => None,
        }
    }
}

impl Display for LinkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_ascii_lowercase())
    }
}

#[derive(Clone, Copy, clap::ValueEnum, Debug)]
enum LinkTo {
    Binary,
    Library,
    Both,
    Either,
}

#[derive(clap::Parser, Debug)]
struct Args {
    /// Optional path to a rust repositiory other than the cwd.
    #[arg(long, short)]
    rust_repo: Option<PathBuf>,

    /// The privacy level that each module will be declared as.
    #[arg(long, short = 'p', default_value_t = LinkKind::Private)]
    privacy_level: LinkKind,

    /// Link with the binary
    #[arg(long = "bin",  action = clap::ArgAction::SetTrue)]
    bin: bool,

    /// Link with the library
    #[arg(long = "lib",  action = clap::ArgAction::SetTrue)]
    lib: bool,
}

impl Args {
    fn link_to(&self) -> LinkTo {
        match (self.lib, self.bin) {
            (true, false) => LinkTo::Library,
            (false, true) => LinkTo::Binary,
            (true, true) => LinkTo::Both,
            (false, false) => LinkTo::Either,
        }
    }
}

fn error_exit(e: impl Display) -> ! {
    println!("\x1b[31;1mError:\x1b[0m {e}");
    exit(1)
}

fn generate_builders(src_folder: PathBuf, name: String, link_to: LinkTo) -> Vec<ModuleBuilder> {
    match link_to {
        LinkTo::Binary => {
            let path = src_folder.join("main.rs");
            if path.exists() {
                vec![ModuleBuilder::new(path, name)]
            } else {
                error_exit(format!(
                    "main.rs could not both be found in source folder `{}`",
                    src_folder.to_string_lossy()
                ))
            }
        }
        LinkTo::Library => {
            let path = src_folder.join("lib.rs");
            if path.exists() {
                vec![ModuleBuilder::new(path, name)]
            } else {
                error_exit(format!(
                    "lib.rs could not both be found in source folder `{}`",
                    src_folder.to_string_lossy()
                ))
            }
        }
        LinkTo::Both => {
            let path_1 = src_folder.join("lib.rs");
            let path_2 = src_folder.join("main.rs");
            if path_1.exists() && path_2.exists() {
                vec![
                    ModuleBuilder::new(path_1, name.clone()),
                    ModuleBuilder::new(path_2, name),
                ]
            } else {
                error_exit(format!(
                    "main.rs and lib.rs could not both be found in source folder `{}`",
                    src_folder.to_string_lossy()
                ))
            }
        }
        LinkTo::Either => {
            let path_1 = src_folder.join("lib.rs");
            let path_2 = src_folder.join("main.rs");
            match (path_1.exists(), path_2.exists()) {
                (true, false) => vec![ModuleBuilder::new(path_1, name)],
                (false, true) => vec![ModuleBuilder::new(path_2, name)],
                (true, true) => vec![
                    ModuleBuilder::new(path_1, name.clone()),
                    ModuleBuilder::new(path_2, name),
                ],
                (false, false) => error_exit(format!(
                    "Neither main.rs or lib.rs could be found in source folder `{}`",
                    src_folder.to_string_lossy()
                )),
            }
        }
    }
}

fn main() {
    let args = Args::parse();

    let link_to = args.link_to();
    let link_kind = args.privacy_level;

    let mut metadata_cmd = MetadataCommand::new();
    if let Some(path) = args.rust_repo {
        metadata_cmd.current_dir(path);
    }
    metadata_cmd.no_deps();

    let metadata = metadata_cmd.exec().map_err(error_exit).unwrap();

    let repos: Vec<(String, PathBuf)> = metadata
        .packages
        .into_iter()
        .map(|p| {
            (
                p.name.to_string(),
                p.manifest_path.parent().unwrap().join(r"src\").into(),
            )
        })
        .collect();

    let builders = repos
        .into_iter()
        .flat_map(|(name, src_folder)| generate_builders(src_folder, name, link_to));

    let modules = builders.map(|b| b.build());

    for mut m in modules.into_iter() {
        m.update_childrens_linked_status();
        println!("before:\n{}\n---\n", m);
        m.link_unlinked_children(link_kind);
        println!("after:\n{}", m)
    }
}
