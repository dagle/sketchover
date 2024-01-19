use std::path::Path;

use calloop::EventLoop;
use mlua::{Function, IntoLuaMulti, Table, Value};
use mlua::{Lua, UserData, UserDataMethods};
use sketchover::mousemap::Mouse;
// use rlua::{Table, Value};
use sketchover::output::OutPut;
use sketchover::runtime::Events;
use sketchover::runtime::Runtime;
use sketchover::tools::draw::pen::Pen;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};
use xdg::BaseDirectories;

struct LuaBindings {
    lua: Lua,
}
// #[derive(Clone, Debug)]
// enum Command {
//     Exit,
// }
//
// struct RuntimeData<'a>(&'a mut Runtime<LuaBindings>);
//
// impl<'a> UserData for RuntimeData<'a> {
//     fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
//         methods.add_method("quit", |_, rt, ()| {
//             rt.0.exit();
//             Ok(())
//         });
//         methods.add_method("undo", |_, rt, ()| {
//             rt.0.exit();
//             Ok(())
//         });
//         methods.add_method("pause", |_, rt, ()| {
//             rt.0.exit();
//             Ok(())
//         });
//         methods.add_method_mut("clear", |_, rt, ()| {
//             rt.0.clear(true);
//             Ok(())
//         });
//         methods.add_method_mut("run", |_, rt, ()| {
//             // rt.0.run();
//             Ok(())
//         });
//     }
// }
//
// ---@class keyevent
// ---@field legs integer
// ---@field eyes integer
// sketchover.on("key", function(keyevent))
//
//
//
//

struct LuaKeyEvent {
    modifiers: Modifiers,
    key: KeyEvent,
}

impl UserData for LuaKeyEvent {
    fn add_fields<'lua, F: mlua::prelude::LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field("modifer", true)
    }
}

impl Events for LuaBindings {
    // fn init(&mut self) {
    //     if let Ok(xdg_dirs) = BaseDirectories::with_prefix("sketchover") {
    //         let config_dir = xdg_dirs.get_config_home();
    //         let path = format!(
    //             "package.path = package.path .. ';{}sketchover.lua'",
    //             config_dir.as_os_str().to_str().unwrap()
    //         );
    //         self.lua.load(&path).exec().unwrap();
    //     }
    // }
    fn new_output(r: &mut Runtime<Self>, ouput: &mut OutPut) {
        let globals = r.data.lua.globals();
    }

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent) {
        let lua = &r.data.lua;
        let modifiers = r.modifiers();

        let args = LuaKeyEvent {
            modifiers,
            key: event,
        };
        emit_sync_callback(lua, ("keypress".to_owned(), args));

        // struct RuntimeData<'a>(&'a mut Runtime<LuaBindings>);
    }

    fn mousebinding(r: &mut Runtime<Self>, button: u32) {}
}

pub fn emit_sync_callback<'lua, A>(
    lua: &'lua Lua,
    (name, args): (String, A),
) -> mlua::Result<mlua::Value<'lua>>
where
    A: IntoLuaMulti<'lua>,
{
    let decorated_name = format!("sketchover-event-{}", name);
    let tbl: mlua::Value = lua.named_registry_value(&decorated_name)?;
    match tbl {
        mlua::Value::Table(tbl) => {
            for func in tbl.sequence_values::<mlua::Function>() {
                let func = func?;
                return func.call(args);
            }
            Ok(mlua::Value::Nil)
        }
        _ => Ok(mlua::Value::Nil),
    }
}

pub fn get_or_create_module<'lua>(lua: &'lua Lua, name: &str) -> anyhow::Result<mlua::Table<'lua>> {
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let loaded: Table = package.get("loaded")?;

    let module = loaded.get(name)?;
    match module {
        Value::Nil => {
            let module = lua.create_table()?;
            loaded.set(name, module.clone())?;
            Ok(module)
        }
        Value::Table(table) => Ok(table),
        wat => anyhow::bail!(
            "cannot register module {} as package.loaded.{} is already set to a value of type {}",
            name,
            name,
            wat.type_name()
        ),
    }
}
pub fn register_event<'lua>(
    lua: &'lua Lua,
    (name, func): (String, mlua::Function),
) -> mlua::Result<()> {
    let register_name = format!("sketchover-event-{}", name);
    let tbl: mlua::Value = lua.named_registry_value(&register_name)?;
    match tbl {
        mlua::Value::Nil => {
            let tbl = lua.create_table()?;
            tbl.set(1, func)?;
            lua.set_named_registry_value(&register_name, tbl)?;
            Ok(())
        }
        mlua::Value::Table(tbl) => {
            let len = tbl.raw_len();
            tbl.set(len + 1, func)?;
            Ok(())
        }
        _ => Err(mlua::Error::external(anyhow::anyhow!(
            "registry key for {} has invalid type",
            &register_name
        ))),
    }
}

pub fn make_lua_context(config_file: &Path) -> anyhow::Result<Lua> {
    let lua = Lua::new();

    let config_dir = config_file.parent().unwrap_or_else(|| Path::new("/"));

    {
        let globals = lua.globals();
        let sketchover = get_or_create_module(&lua, "sketchover")?;

        sketchover.set("");
        sketchover.set("on", lua.create_function(register_event)?)?;
    }

    Ok(lua)
}

fn main() -> anyhow::Result<()> {
    if let Ok(xdg_dirs) = BaseDirectories::with_prefix("sketchover") {
        let mut config = xdg_dirs.get_config_home();
        config.push("init.lua");
        let lua = make_lua_context(config.as_path())?;
        let binds = LuaBindings { lua };

        let mut rt = Runtime::init(binds);
        let event_loop = EventLoop::try_new().expect("couldn't create event-loop");
        rt.run(event_loop);
        Ok(())
    } else {
        Ok(())
    }
}
