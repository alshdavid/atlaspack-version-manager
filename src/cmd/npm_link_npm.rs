use std::fs;
use std::fs::Permissions;

use fs_extra::dir::CopyOptions;
use log::info;

use super::npm_link::NpmLinkCommand;
use crate::context::Context;
use crate::platform::link;
use crate::platform::package::PackageDescriptor;
use crate::platform::path_ext::*;

pub fn npm_link_npm(
  ctx: Context,
  _cmd: NpmLinkCommand,
  package: PackageDescriptor,
) -> anyhow::Result<()> {
  let package_lib = package.path_real()?;
  let package_lib_static = package_lib.join("lib");

  // node_modules
  let node_modules = ctx.env.pwd.join("node_modules");
  let node_modules_bin = node_modules.join(".bin");

  // node_modules/.bin
  #[cfg(unix)]
  let node_modules_bin_atlaspack = node_modules_bin.join("atlaspack");
  #[cfg(windows)]
  let node_modules_bin_atlaspack = node_modules_bin.join("atlaspack.exe");

  // node_modules/atlaspack
  let node_modules_super = node_modules.join("atlaspack");

  // node_modules/@atlaspack
  let node_modules_atlaspack = node_modules.join("@atlaspack");

  // Create node_modules if it doesn't exist
  if !fs::exists(&node_modules)? {
    info!("Deleting: {:?}", node_modules);
    fs::create_dir_all(&node_modules)?;
  }
  if !fs::exists(&node_modules_bin)? {
    info!("Deleting: {:?}", node_modules_bin);
    fs::create_dir_all(&node_modules_bin)?;
  }

  // Delete existing node_modules/.bin/atlaspack
  if fs::exists(&node_modules_bin_atlaspack)? {
    info!("Deleting: {:?}", node_modules_bin_atlaspack);
    fs::remove_file(&node_modules_bin_atlaspack)?;
  }

  // node_modules/atlaspack
  if fs::exists(&node_modules_super)? {
    info!("Deleting: {:?}", node_modules_super);
    fs::remove_dir_all(&node_modules_super)?;
  }
  fs::create_dir_all(&node_modules_super)?;

  // Create node_modules/@atlaspack
  if fs::exists(&node_modules_atlaspack)? {
    info!("Deleting: {:?}", node_modules_atlaspack);
    fs::remove_dir_all(&node_modules_atlaspack)?
  }
  fs::create_dir_all(&node_modules_atlaspack)?;

  info!("Copying: {:?} {:?}", package_lib, node_modules_super);

  fs_extra::dir::copy(
    &package_lib,
    &node_modules_super,
    &CopyOptions {
      content_only: true,
      ..Default::default()
    },
  )?;

  link::soft_link(&node_modules_super, &node_modules_atlaspack.join("super"))?;

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;

    info!("Creating: node_modules/.bin/atlaspack");
    fs::write(
      &node_modules_bin_atlaspack,
      "#!/usr/bin/env node\nrequire('atlaspack/lib/cli.js')\n",
    )?;
    fs::set_permissions(&node_modules_bin_atlaspack, Permissions::from_mode(0o777))?;
  }

  #[cfg(windows)]
  {
    // Just use a wrapper process for Windows
    crate::platform::link::hard_link_or_copy(&ctx.env.exe_path, &node_modules_bin_atlaspack)?;
  }

  for entry in fs::read_dir(&package_lib_static)? {
    let entry = entry?;
    let entry_path = entry.path();

    if fs::metadata(&entry_path)?.is_dir() {
      continue;
    }

    let file_stem = entry_path.try_file_stem()?;

    if file_stem.starts_with("vendor.") {
      continue;
    }

    let node_modules_atlaspack_pkg = node_modules_atlaspack.join(&file_stem);
    if fs::exists(&node_modules_atlaspack_pkg)? {
      info!("Deleting: {:?}", node_modules_atlaspack_pkg);
      fs::remove_dir_all(&node_modules_atlaspack_pkg)?;
    }

    fs::create_dir(&node_modules_atlaspack_pkg)?;
    fs::write(
      node_modules_atlaspack_pkg.join("package.json"),
      (json::object! {
        "name": format!("@atlaspack/{file_stem}"),
        "main": "./index.js",
        "type": "commonjs"
      })
      .pretty(2),
    )?;

    if file_stem == "runtime-js" {
      fs::create_dir_all(node_modules_atlaspack_pkg.join("lib"))?;
      fs_extra::dir::copy(
        package_lib_static
          .join("runtimes")
          .join("js")
          .join("helpers"),
        node_modules_atlaspack_pkg.join("lib"),
        &CopyOptions::default(),
      )?;
    }

    fs::write(
      node_modules_atlaspack_pkg.join("index.js"),
      format!("module.exports = require('atlaspack/lib/{file_stem}.js')\n"),
    )?;
  }

  Ok(())
}
