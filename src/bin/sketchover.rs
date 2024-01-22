use std::path::Path;
use std::rc::Rc;
use std::sync;

use calloop::channel::SyncSender;
use calloop::EventLoop;
use mlua::{Function, IntoLuaMulti, Table, Value};
use mlua::{Lua, UserData, UserDataMethods};
use sketchover::output::OutPut;
use sketchover::runtime::Events;
use sketchover::runtime::Runtime;
use sketchover::tools::draw::line::Line;
use sketchover::tools::draw::pen::Pen;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};
use xdg::BaseDirectories;

struct LuaBindings {
    lua: Rc<Lua>,
    sender: Option<Rc<SyncSender<Message>>>,
}

enum Message {
    Clear,
    Quit,
    Undo,
    Pause,
    Unpause,

    Drawing(String, (f64, f64)),
}

impl LuaBindings {
    fn new(lua: Rc<Lua>) -> Self {
        LuaBindings { lua, sender: None }
    }
}

struct RuntimeData(Runtime<LuaBindings>);

impl UserData for RuntimeData {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_function("keypress", |lua, func: Function| {
            register_event(lua, ("keypress".to_owned(), func))?;
            Ok(())
        });
        methods.add_function("new_output", |lua, func: Function| {
            register_event(lua, ("new_output".to_owned(), func))?;
            Ok(())
        });
        methods.add_function("mousepress", |lua, func: Function| {
            register_event(lua, ("mousepress".to_owned(), func))?;
            Ok(())
        });
        // methods.add_function("remove_output", |lua, func: Function| {
        //     register_event(lua, ("remove_output".to_owned(), func))?;
        //     Ok(())
        // });

        methods.add_method_mut("run", |_, rt, ()| {
            let event_loop = EventLoop::try_new().expect("couldn't create event-loop");
            let (sender, receiver) = calloop::channel::sync_channel::<Message>(1);
            event_loop
                .handle()
                .insert_source(
                    receiver,
                    |event, _, rt: &mut Runtime<LuaBindings>| match event {
                        calloop::channel::Event::Msg(m) => match m {
                            Message::Clear => rt.clear(true),
                            Message::Quit => rt.exit(),
                            Message::Undo => rt.undo(),
                            Message::Unpause => rt.set_pause(false),
                            Message::Pause => rt.set_pause(true),
                            Message::Drawing(s, pos) => {
                                match s.as_ref() {
                                    "pen" => rt.start_drawing(Box::new(Pen::new())),
                                    // "box" => rt.start_drawing(Box::new(Line::new())),
                                    "line" => rt.start_drawing(Box::new(Line::new(pos))),
                                    _ => panic!("tool doesn't exist"),
                                }
                            }
                        },
                        calloop::channel::Event::Closed => {
                            rt.exit();
                        }
                    },
                )
                .unwrap();
            rt.0.data.sender = Some(Rc::new(sender));
            rt.0.run(event_loop);
            Ok(())
        });
    }
}

// pub fn handler(event: Event)
// F: FnMut(S::Event, &mut S::Metadata, &mut Data) -> S::Ret + 'l,

struct LuaKeyEvent {
    modifiers: Modifiers,
    key: KeyEvent,
}

impl UserData for LuaKeyEvent {
    fn add_fields<'lua, F: mlua::prelude::LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("key", |_, event| Ok(event.key.keysym.name()));
        fields.add_field_method_get("modifiers", |lua, event| {
            let table = lua.create_table()?;
            table.set("ctrl", event.modifiers.ctrl)?;
            table.set("alt", event.modifiers.ctrl)?;
            table.set("shift", event.modifiers.ctrl)?;
            table.set("caps_lock", event.modifiers.ctrl)?;
            Ok(table)
        });
    }
}

struct MouseEvent {
    modifiers: Modifiers,
    button: u32,
    pos: (f64, f64),
}

impl UserData for MouseEvent {
    fn add_fields<'lua, F: mlua::prelude::LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("button", |_, event| Ok(event.button));
        fields.add_field_method_get("pos", |lua, event| {
            //
            let table = lua.create_table()?;
            table.set("x", event.pos.0)?;
            table.set("y", event.pos.1)?;
            Ok(table)
        });
        fields.add_field_method_get("modifiers", |lua, event| {
            let table = lua.create_table()?;
            table.set("ctrl", event.modifiers.ctrl)?;
            table.set("alt", event.modifiers.ctrl)?;
            table.set("shift", event.modifiers.ctrl)?;
            table.set("caps_lock", event.modifiers.ctrl)?;
            Ok(table)
        });
    }
}

struct Callback {
    sender: Rc<SyncSender<Message>>,
}

impl UserData for Callback {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("quit", |_, cb, ()| {
            cb.sender.send(Message::Quit).unwrap();
            Ok(())
        });
        methods.add_method("clear", |_, cb, ()| {
            cb.sender.send(Message::Clear).unwrap();
            Ok(())
        });
        methods.add_method("undo", |_, cb, ()| {
            cb.sender.send(Message::Undo).unwrap();
            Ok(())
        });
        methods.add_method("pause", |_, cb, ()| {
            cb.sender.send(Message::Pause).unwrap();
            Ok(())
        });
        methods.add_method("unpause", |_, cb, ()| {
            cb.sender.send(Message::Unpause).unwrap();
            Ok(())
        });

        methods.add_method("draw", |_, cb, (name, table): (String, Table)| {
            let x = table.get("x")?;
            let y = table.get("y")?;
            cb.sender.send(Message::Drawing(name, (x, y))).unwrap();
            Ok(())
        });
    }
}

impl Events for LuaBindings {
    fn new_output(r: &mut Runtime<Self>, ouput: &mut OutPut) {
        let lua = &r.data.lua.clone();
        let globals = r.data.lua.globals();

        // emit_sync_callback(lua, ("new_output".to_owned(), args));
    }

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent) {
        let lua = &r.data.lua.clone();
        let modifiers = r.modifiers();

        let args = LuaKeyEvent {
            modifiers,
            key: event,
        };
        let cb = Callback {
            sender: r.data.sender.as_ref().unwrap().clone(),
        };

        emit_sync_callback(lua, ("keypress".to_owned(), (cb, args))).expect("callback failed");
    }
    fn mousebinding(r: &mut Runtime<Self>, button: u32) {
        let lua = &r.data.lua.clone();
        let modifiers = r.modifiers();
        let pos = r.pos();

        let args = MouseEvent {
            modifiers,
            button,
            pos,
        };
        let cb = Callback {
            sender: r.data.sender.as_ref().unwrap().clone(),
        };

        // r.start_drawing(Box::new(Pen::new()));
        emit_sync_callback(lua, ("mousepress".to_owned(), (cb, args))).expect("callback failed");
    }
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

pub fn get_or_create_module(lua: Rc<Lua>, name: &str) -> anyhow::Result<()> {
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let loaded: Table = package.get("loaded")?;

    let module = loaded.get(name)?;
    match module {
        Value::Nil => {
            let binds = LuaBindings::new(lua.clone());

            let rt = Runtime::init(binds);
            let data = RuntimeData(rt);
            loaded.set("sketchover", data)?;
            Ok(())
        }
        Value::UserData(_) => Ok(()),
        wat => anyhow::bail!(
            "cannot register module {} as package.loaded.{} is already set to a value of type {}",
            name,
            name,
            wat.type_name()
        ),
    }
}

pub fn make_lua_context(config_file: &Path) -> anyhow::Result<()> {
    let lua = Rc::new(Lua::new());

    get_or_create_module(lua.clone(), "sketchover")?;
    let path = format!(
        "package.path = package.path .. ';{}?.lua;'",
        config_file.to_str().unwrap()
    );
    println!("path: {}", &path);
    lua.load(&path).exec().unwrap();
    lua.load("require('mmm')").exec().unwrap();

    Ok(())
}

fn main() -> anyhow::Result<()> {
    if let Ok(xdg_dirs) = BaseDirectories::with_prefix("sketchover") {
        let config = xdg_dirs.get_config_home();
        make_lua_context(config.as_path())?;
    }
    Ok(())
}
