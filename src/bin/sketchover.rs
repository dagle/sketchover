use std::path::Path;
use std::rc::Rc;

use calloop::channel::SyncSender;
use calloop::EventLoop;
use mlua::{Error, Function, IntoLuaMulti, Table, Value};
use mlua::{Lua, UserData, UserDataMethods};
use raqote::{LineCap, LineJoin, SolidSource, StrokeStyle};
use sketchover::output::OutPut;
use sketchover::runtime::Events;
use sketchover::runtime::Runtime;
use sketchover::tools::draw::draw::Draw;
use sketchover::tools::draw::line::Line;
use sketchover::tools::draw::pen::Pen;
use sketchover::tools::draw::rekt::Rect;
use smithay_client_toolkit::output::OutputInfo;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};
use wayland_client::protocol::wl_output::{Subpixel, Transform};
use xdg::BaseDirectories;

struct LuaBindings {
    lua: Rc<Lua>,
    sender: Option<Rc<SyncSender<Message>>>,
}

enum Message {
    Clear(Option<u32>),
    Quit,
    Undo(Option<u32>),
    Pause(Option<u32>),
    Unpause(Option<u32>),
    Save(Option<u32>),

    SetFg(SolidSource, Option<u32>),
    StopDraw,
    Drawing(String, (f64, f64), Draw),
}

impl LuaBindings {
    fn new(lua: Rc<Lua>) -> Self {
        LuaBindings { lua, sender: None }
    }
}
macro_rules! set {
    ($field:expr, $result:expr) => {
        if let Ok(value) = $result {
            $field = value;
        }
    };
}

struct RuntimeData(Runtime<LuaBindings>);

impl UserData for RuntimeData {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_function("init", |lua, func: Function| {
            register_event(lua, ("init".to_owned(), func))?;
            Ok(())
        });
        methods.add_function("keypress", |lua, func: Function| {
            register_event(lua, ("keypress".to_owned(), func))?;
            Ok(())
        });
        methods.add_function("new_output", |lua, func: Function| {
            register_event(lua, ("new_output".to_owned(), func))?;
            Ok(())
        });
        methods.add_function("destroy_output", |lua, func: Function| {
            register_event(lua, ("destroy_output".to_owned(), func))?;
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
            let (sender, receiver) = calloop::channel::sync_channel::<Message>(3);
            event_loop
                .handle()
                .insert_source(
                    receiver,
                    |event, _, rt: &mut Runtime<LuaBindings>| match event {
                        calloop::channel::Event::Msg(m) => match m {
                            Message::Clear(id) => {
                                let output = rt.locate_output(id).expect("couldn't find screen");
                                output.clear();
                            }
                            Message::Quit => rt.exit(),
                            Message::Undo(id) => {
                                let output = rt.locate_output(id).unwrap();
                                output.undo();
                            }
                            Message::Unpause(id) => {
                                let id = rt
                                    .locate_output_idx(id)
                                    .expect("Can't find screen to unpause");
                                rt.set_pause(false, id);
                            }

                            Message::Pause(id) => {
                                let id = rt
                                    .locate_output_idx(id)
                                    .expect("Can't find screen to pause");
                                rt.set_pause(true, id);
                            }
                            Message::SetFg(solid, id) => {
                                let output = rt.locate_output(id).unwrap();
                                output.set_fg(solid);
                            }
                            Message::Save(id) => {
                                let output = rt.locate_output(id).unwrap();
                                output.save("sketchover").expect("couldn't save output");
                            }
                            Message::StopDraw => rt.stop_drawing(),
                            Message::Drawing(s, pos, draw) => {
                                // println!("draw: {:?}", draw);
                                match s.as_ref() {
                                    "pen" => rt.start_drawing(Box::new(Pen::new(pos, draw))),
                                    "rect" => rt.start_drawing(Box::new(Rect::new(pos, draw))),
                                    "line" => rt.start_drawing(Box::new(Line::new(pos, draw))),
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

struct LuaKeyEvent {
    modifiers: Modifiers,
    key: KeyEvent,
    pos: (f64, f64),
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
        fields.add_field_method_get("pos", |lua, event| {
            let table = lua.create_table()?;
            table.set("x", event.pos.0)?;
            table.set("y", event.pos.1)?;
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
        fields.add_field_method_get("button", |lua, event| {
            let value = match event.button {
                0x110 => lua.create_string("left"),
                0x111 => lua.create_string("right"),
                0x112 => lua.create_string("middle"),
                0x113 => lua.create_string("side"),
                0x114 => lua.create_string("extra"),
                0x115 => lua.create_string("forward"),
                0x116 => lua.create_string("back"),
                0x117 => lua.create_string("task"),
                raw => return Ok(Value::Number(raw.into())),
            };
            let k = value?;
            Ok(Value::String(k))
        });
        fields.add_field_method_get("pos", |lua, event| {
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
    screen_id: Option<u32>,
}

impl Callback {
    fn screen_id(&self, value: Value) -> mlua::Result<Option<u32>> {
        match value {
            Value::Nil => Ok(None),
            Value::Number(n) => Ok(Some(n as u32)),
            wat => Err(Error::RuntimeError(format!(
                "Expected number or nil, got: {}",
                wat.type_name()
            ))),
        }
    }
}

impl UserData for Callback {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("quit", |_, cb, ()| {
            cb.sender.send(Message::Quit).unwrap();
            Ok(())
        });
        methods.add_method("clear", |_, cb, value| {
            let id = cb.screen_id(value)?;
            cb.sender.send(Message::Clear(id)).unwrap();
            Ok(())
        });
        methods.add_method("undo", |_, cb, value| {
            let id = cb.screen_id(value)?;
            cb.sender.send(Message::Undo(id)).unwrap();
            Ok(())
        });
        methods.add_method("pause", |_, cb, value| {
            let id = cb.screen_id(value)?;
            cb.sender.send(Message::Pause(id)).unwrap();
            Ok(())
        });
        methods.add_method("unpause", |_, cb, value| {
            let id = cb.screen_id(value)?;
            cb.sender.send(Message::Unpause(id)).unwrap();
            Ok(())
        });
        methods.add_method("set_fg", |_, cb, (color_value, id)| {
            let mut color = SolidSource {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };

            let id = cb.screen_id(id)?;
            lua_color(&mut color, color_value)?;

            cb.sender.send(Message::SetFg(color, id)).unwrap();
            Ok(())
        });

        methods.add_method("save", |_, cb, id| {
            let id = cb.screen_id(id)?;
            cb.sender.send(Message::Save(id)).unwrap();
            Ok(())
        });

        methods.add_method("stop_draw", |_, cb, ()| {
            cb.sender.send(Message::StopDraw).unwrap();
            Ok(())
        });

        methods.add_method(
            "draw",
            |_, cb, (name, table, draw): (String, Table, Table)| {
                let x = table.get("x")?;
                let y = table.get("y")?;
                let draw = table_to_draw(draw)?;
                cb.sender
                    .send(Message::Drawing(name, (x, y), draw))
                    .unwrap();
                Ok(())
            },
        );
    }
}

struct LuaOutPut(OutputInfo);

impl UserData for LuaOutPut {
    fn add_fields<'lua, F: mlua::prelude::LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, info| Ok(info.0.id));
        fields.add_field_method_get("name", |_, info| Ok(info.0.name.clone()));
        fields.add_field_method_get("model", |_, info| Ok(info.0.model.clone()));
        fields.add_field_method_get("make", |_, info| Ok(info.0.make.clone()));
        fields.add_field_method_get("location", |lua, info| {
            let table = lua.create_table()?;
            table.set(1, info.0.location.0)?;
            table.set(2, info.0.location.1)?;
            Ok(table)
        });
        fields.add_field_method_get("physical_size", |lua, info| {
            let table = lua.create_table()?;
            table.set(1, info.0.physical_size.0)?;
            table.set(2, info.0.physical_size.1)?;
            Ok(table)
        });
        fields.add_field_method_get("subpixel", |_, info| {
            let str = match info.0.subpixel {
                Subpixel::Unknown => "unknown",
                Subpixel::None => "none",
                Subpixel::HorizontalRgb => "HorizontalRgb",
                Subpixel::HorizontalBgr => "HorizontalBgr",
                Subpixel::VerticalRgb => "VerticalRgb",
                Subpixel::VerticalBgr => "VerticalBgr",
                _ => todo!(),
            };
            Ok(str.to_owned())
        });
        fields.add_field_method_get("transform", |_, info| {
            let str = match info.0.transform {
                Transform::Normal => "normal",
                Transform::_90 => "90",
                Transform::_180 => "180",
                Transform::_270 => "270",
                Transform::Flipped => "flipped",
                Transform::Flipped90 => "flipped90",
                Transform::Flipped180 => "flipped180",
                Transform::Flipped270 => "flipped180",
                _ => todo!(),
            };
            Ok(str.to_owned())
        });
        fields.add_field_method_get("scale_facor", |_, info| Ok(info.0.scale_factor));
        // fields.add_field_method_get("modes", |_, info| Ok(info.0.make.clone()));
        fields.add_field_method_get("logical_position", |lua, info| {
            match info.0.logical_position {
                None => Ok(Value::Nil),
                Some(p) => {
                    let table = lua.create_table()?;
                    table.set(1, p.0)?;
                    table.set(2, p.1)?;
                    Ok(Value::Table(table))
                }
            }
        });
        fields.add_field_method_get("logical_size", |lua, info| match info.0.logical_size {
            None => Ok(Value::Nil),
            Some(p) => {
                let table = lua.create_table()?;
                table.set(1, p.0)?;
                table.set(2, p.1)?;
                Ok(Value::Table(table))
            }
        });
        fields.add_field_method_get("description", |_, info| Ok(info.0.description.clone()));
    }
}

impl Events for LuaBindings {
    fn init(r: &mut Runtime<Self>) {
        let lua = &r.data.lua.clone();
        let id = r.current_output_id();

        let cb = Callback {
            sender: r.data.sender.as_ref().unwrap().clone(),
            screen_id: id,
        };

        emit_sync_callback(lua, ("init".to_owned(), cb)).expect("callback failed");
    }

    fn destroy_output(r: &mut Runtime<Self>, output_id: u32) {
        let lua = &r.data.lua.clone();
        let id = r.current_output_id();

        let cb = Callback {
            sender: r.data.sender.as_ref().unwrap().clone(),
            screen_id: id,
        };

        emit_sync_callback(lua, ("destroy_output".to_owned(), (cb, output_id)))
            .expect("callback failed");
    }

    fn new_output(r: &mut Runtime<Self>, output: &mut OutPut) {
        let lua = &r.data.lua.clone();
        let id = r.current_output_id();

        let cb = Callback {
            sender: r.data.sender.as_ref().unwrap().clone(),
            screen_id: id,
        };

        let args = LuaOutPut(output.info.clone());

        emit_sync_callback(lua, ("new_output".to_owned(), (cb, args))).expect("callback failed");
    }

    fn keybinding(r: &mut Runtime<Self>, event: KeyEvent, press: bool) {
        let lua = &r.data.lua.clone();
        let modifiers = r.modifiers();
        let pos = r.pos();
        let id = r.current_output_id();

        let args = LuaKeyEvent {
            modifiers,
            key: event,
            pos,
        };

        let cb = Callback {
            sender: r.data.sender.as_ref().unwrap().clone(),
            screen_id: id,
        };

        emit_sync_callback(lua, ("keypress".to_owned(), (cb, args, press)))
            .expect("callback failed");
    }
    fn mousebinding(r: &mut Runtime<Self>, button: u32, press: bool) {
        let lua = &r.data.lua.clone();
        let modifiers = r.modifiers();
        let pos = r.pos();
        let id = r.current_output_id();

        let args = MouseEvent {
            modifiers,
            button,
            pos,
        };
        let cb = Callback {
            sender: r.data.sender.as_ref().unwrap().clone(),
            screen_id: id,
        };

        // r.start_drawing(Box::new(Pen::new()));
        emit_sync_callback(lua, ("mousepress".to_owned(), (cb, args, press)))
            .expect("callback failed");
    }
}
pub fn table_to_draw(table: Table) -> mlua::Result<Draw> {
    let mut default = Draw::default();
    if let Ok(v) = table.get("style") {
        lua_style(&mut default.style, v)?;
    }
    if let Ok(v) = table.get("color") {
        lua_color(&mut default.color, v)?;
    }
    Ok(default)
}

pub fn lua_style(style: &mut StrokeStyle, value: Value) -> mlua::Result<()> {
    match value {
        Value::Nil => Ok(()),
        Value::Table(t) => {
            set!(style.width, t.get("width"));

            set!(style.cap, t.get("cap").map(|x: String| read_linecap(&x))?);
            set!(
                style.join,
                t.get("join").map(|x: String| read_linejoin(&x))?
            );

            set!(style.miter_limit, t.get("miter_limit"));
            set!(style.dash_array, t.get("dash_array"));
            set!(style.dash_offset, t.get("dash_offset"));
            Ok(())
        }
        _ => Err(Error::FromLuaConversionError {
            from: "string",
            to: "LineJoin",
            message: None,
        }),
    }
}

pub fn lua_color(color: &mut SolidSource, value: Value) -> mlua::Result<()> {
    match value {
        Value::Nil => Ok(()),
        Value::Table(t) => {
            set!(color.r, t.get("r"));
            set!(color.g, t.get("g"));
            set!(color.b, t.get("b"));
            set!(color.a, t.get("a"));
            Ok(())
        }
        _ => Err(Error::FromLuaConversionError {
            from: "string",
            to: "LineJoin",
            message: None,
        }),
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
        Value::Table(tbl) => {
            if let Some(func) = tbl.sequence_values::<mlua::Function>().next() {
                let func = func?;
                return func.call(args);
            }
            Ok(mlua::Value::Nil)
        }
        _ => Ok(mlua::Value::Nil),
    }
}

pub fn register_event(lua: &Lua, (name, func): (String, mlua::Function)) -> mlua::Result<()> {
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

pub fn get_or_create_runtime(lua: Rc<Lua>, name: &str) -> anyhow::Result<()> {
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let loaded: Table = package.get("loaded")?;

    let module = loaded.get(name)?;
    match module {
        Value::Nil => {
            let binds = LuaBindings::new(lua.clone());

            let rt = Runtime::init(binds);
            let data = RuntimeData(rt);
            loaded.set(name, data)?;
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

fn read_linecap(m: &str) -> mlua::Result<LineCap> {
    let res = match m {
        "round" => LineCap::Round,
        "square" => LineCap::Square,
        "butt" => LineCap::Butt,
        v => {
            return Err(Error::FromLuaConversionError {
                from: "string",
                to: "LineCap",
                message: Some(v.to_owned()),
            })
        }
    };
    Ok(res)
}

fn read_linejoin(m: &str) -> mlua::Result<LineJoin> {
    let res = match m {
        "round" => LineJoin::Round,
        "square" => LineJoin::Miter,
        "butt" => LineJoin::Bevel,
        v => {
            return Err(Error::FromLuaConversionError {
                from: "string",
                to: "LineJoin",
                message: Some(v.to_owned()),
            })
        }
    };
    Ok(res)
}

pub fn make_lua_context(config_file: &Path) -> anyhow::Result<()> {
    let lua = Rc::new(Lua::new());

    get_or_create_runtime(lua.clone(), "sketchover")?;
    let path = format!(
        "package.path = package.path .. ';{}?.lua;'",
        config_file.to_str().unwrap()
    );
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
