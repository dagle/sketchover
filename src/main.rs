use rlua::{Lua, Result, UserData, UserDataMethods};
use rlua::{Table, Value};
use sketchover::output::OutPut;
use sketchover::runtime::Events;
use sketchover::runtime::Runtime;
use sketchover::tools::draw::pen::Pen;
use smithay_client_toolkit::seat::keyboard::KeyEvent;
use xdg::BaseDirectories;

struct LuaBindings {
    lua: Lua,
}

impl LuaBindings {
    fn map_key(&self, key: &str) -> Result<()> {
        self.lua.context(|lua_ctx| {
            let globals = lua_ctx.globals();
            let tbl: Table = globals.get("Sketchover")?;
            let key_map: Table = tbl.get("key_map")?;
            for pair in key_map.pairs::<i64, Table>() {
                let (_, value) = pair?;
                let bind: String = value.get("key")?;
                if bind == key {
                    println!("Found key!");
                }
                // for pair in value.pairs::<String, Value>() {
                //     let (key, value) = pair?;
                //     println!("key: {:?}, value: {:?}", key, value);
                // }
                // println!("");
            }
            // let scroll_treshold: f64 = tbl.get("key_map")?;
            // println!("{}", scroll_treshold);
            Ok(())
        })?;
        Ok(())
    }
}

struct RuntimeData<'a>(&'a Runtime<LuaBindings>);

impl<'a> UserData for RuntimeData<'a> {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("quit", |_, rt, ()| {
            rt.0.exit();
            Ok(())
        });
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("undo", |_, rt, ()| {
            rt.0.exit();
            Ok(())
        });
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("pause", |_, rt, ()| {
            rt.0.exit();
            Ok(())
        });
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("clear", |_, rt, ()| {
            rt.0.exit();
            Ok(())
        });
    }
}

// impl UserData for Runtime<LuaBindings> {
//     fn add_methods<'lua, T: rlua::prelude::LuaUserDataMethods<'lua, Self>>(_methods: &mut T) {}
//
//     fn get_uvalues_count(&self) -> std::os::raw::c_int {
//         1
//     }
// }

impl Events for LuaBindings {
    fn new_output(r: &mut Runtime<Self>, ouput: &OutPut) {
        // println!("New output!")
    }

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent) {
        let key = xkbcommon::xkb::keysym_get_name(event.keysym);

        let apa: Result<()> = r.data.lua.context(|lua_ctx| {
            let globals = lua_ctx.globals();
            let tbl: Table = globals.get("Sketchover")?;
            let key_map: Table = tbl.get("key_map")?;
            for pair in key_map.pairs::<i64, Table>() {
                let (_, value) = pair?;
                let bind: String = value.get("key")?;
                if bind == key {
                    println!("Found key!");
                    r.exit();
                }
            }
            Ok(())
        });
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
        lua.context(|lua_ctx| {
            lua_ctx.load(&path).exec()?;
            lua_ctx.load("require 'sketchover'").exec()?;
            // let globals = lua_ctx.globals();
            // let tbl: Table = globals.get("Sketchover")?;
            // let key_map: Table = tbl.get("key_map")?;
            // for pair in key_map.pairs::<i64, Table>() {
            //     let (key, value) = pair?;
            //     for pair in value.pairs::<String, Value>() {
            //         let (key, value) = pair?;
            //         println!("key: {:?}, value: {:?}", key, value);
            //     }
            //     println!("");
            // }
            // let scroll_treshold: f64 = tbl.get("key_map")?;
            // println!("{}", scroll_treshold);
            Ok(())
        })?;
    }

    let apa = LuaBindings { lua };
    Runtime::init(apa);
    Ok(())
}
