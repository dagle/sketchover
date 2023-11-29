use std::cell::OnceCell;
use std::sync::Mutex;
use std::sync::OnceLock;

use mlua::{Lua, Result, UserData, UserDataMethods};
// use rlua::{Table, Value};
use sketchover::output::OutPut;
use sketchover::runtime::Events;
use sketchover::runtime::Runtime;
use sketchover::tools::draw::pen::Pen;
use smithay_client_toolkit::seat::keyboard::KeyEvent;
use xdg::BaseDirectories;

struct LuaBindings {}

struct RuntimeData(Runtime<LuaBindings>);

impl UserData for RuntimeData {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("quit", |_, rt, ()| {
            rt.0.exit();
            Ok(())
        });
        methods.add_method("undo", |_, rt, ()| {
            rt.0.exit();
            Ok(())
        });
        methods.add_method("pause", |_, rt, ()| {
            rt.0.exit();
            Ok(())
        });
        methods.add_method_mut("clear", |_, rt, ()| {
            rt.0.clear(true);
            Ok(())
        });
        methods.add_method_mut("run", |_, rt, ()| {
            rt.0.run();
            Ok(())
        });
    }
}

impl Events for LuaBindings {
    fn new_output(r: &mut Runtime<Self>, ouput: &OutPut) {}

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent) {
        let key = xkbcommon::xkb::keysym_get_name(event.keysym);
        println!("key: {}", key);
    }

    fn mousebinding(r: &mut Runtime<Self>, button: u32) {
        r.start_drawing(Box::new(Pen::new()));
    }
}

fn main() -> Result<()> {
    let lua = Lua::new();

    if let Ok(xdg_dirs) = BaseDirectories::with_prefix("sketchover") {
        let config_dir = xdg_dirs.get_config_home();
        let path = format!(
            "package.path = package.path .. ';{}sketchover.lua'",
            config_dir.as_os_str().to_str().unwrap()
        );
        lua.load(&path).exec()?;
        let globals = lua.globals();
        let constructor =
            lua.create_function(|_, ()| Ok(RuntimeData(Runtime::init(LuaBindings {}))))?;
        globals.set("runtime", constructor)?;
        lua.load("require 'sketchover'").exec()?;
    }
    Ok(())
}
