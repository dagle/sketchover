use mlua::Function;
use mlua::{Lua, UserData, UserDataMethods};
// use rlua::{Table, Value};
use sketchover::output::OutPut;
use sketchover::runtime::Events;
use sketchover::runtime::Runtime;
use sketchover::tools::draw::pen::Pen;
use smithay_client_toolkit::seat::keyboard::KeyEvent;
use xdg::BaseDirectories;

struct LuaBindings {
    lua: Lua,
}

#[derive(Clone, Debug)]
enum Command {
    Exit,
}

struct RuntimeData<'a>(&'a mut Runtime<LuaBindings>);

impl<'a> UserData for RuntimeData<'a> {
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
            // rt.0.run();
            Ok(())
        });
    }
}

impl Events for LuaBindings {
    fn init(&mut self) {
        if let Ok(xdg_dirs) = BaseDirectories::with_prefix("sketchover") {
            let config_dir = xdg_dirs.get_config_home();
            let path = format!(
                "package.path = package.path .. ';{}sketchover.lua'",
                config_dir.as_os_str().to_str().unwrap()
            );
            self.lua.load(&path).exec().unwrap();
        }
    }
    fn new_output(r: &mut Runtime<Self>, ouput: &mut OutPut) {
        let globals = r.data.lua.globals();
        let fun: Function = globals.get("new_output").unwrap();
        fun.call::<_, ()>("");
    }

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent) {

        // struct RuntimeData<'a>(&'a mut Runtime<LuaBindings>);
    }

    fn mousebinding(r: &mut Runtime<Self>, button: u32) {}
}

fn main() -> Result<(), ()> {
    let lua = Lua::new();
    let b = LuaBindings { lua };
    let mut rt = Runtime::init(b);
    // rt.run();
    Ok(())
}
