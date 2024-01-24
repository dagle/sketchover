use std::path::Path;
use std::rc::Rc;

use calloop::channel::SyncSender;
use calloop::EventLoop;
use mlua::{Error, FromLua, Function, IntoLua, IntoLuaMulti, Number, Table, Value};
use mlua::{Lua, UserData, UserDataMethods};
use raqote::{LineCap, LineJoin, SolidSource, StrokeStyle};
use sketchover::output::OutPut;
use sketchover::runtime::Events;
use sketchover::runtime::Runtime;
use sketchover::tools::draw::draw::Draw;
use sketchover::tools::draw::line::Line;
use sketchover::tools::draw::pen::Pen;
use sketchover::tools::draw::rekt::Rect;
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

    Drawing(String, (f64, f64), LuaDraw),
}

impl LuaBindings {
    fn new(lua: Rc<Lua>) -> Self {
        LuaBindings { lua, sender: None }
    }
}

#[derive(Clone, FromLua)]
struct LuaDraw {
    color: LuaColor,
    style: LuaStyle,
}

impl From<LuaDraw> for Draw {
    fn from(value: LuaDraw) -> Self {
        Draw {
            style: value.style.0,
            color: value.color.0,
        }
    }
}

impl Default for LuaDraw {
    fn default() -> Self {
        let style = LuaStyle::default();
        let color = LuaColor::default();
        LuaDraw { color, style }
    }
}

impl LuaDraw {
    pub fn lua_clone<'lua>(&self, value: Value<'lua>) -> mlua::Result<Self> {
        let d = match value {
            Value::Nil => self.clone(),
            Value::Table(t) => {
                let color = t.get("color").unwrap_or(self.color.clone());
                let style = t.get("style").unwrap_or(self.style.clone());
                LuaDraw { color, style }
            }
            _ => {
                return Err(Error::FromLuaConversionError {
                    from: "string",
                    to: "LineJoin",
                    message: None,
                })
            }
        };
        Ok(d)
    }
}

impl UserData for LuaDraw {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("clone", |_, draw, arg| draw.lua_clone(arg));
    }
    fn add_fields<'lua, F: mlua::prelude::LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("color", |_, draw| Ok(draw.color.clone()));
        fields.add_field_method_get("style", |_, draw| Ok(draw.style.clone()));
    }
}

#[derive(Clone, Default, FromLua)]
struct LuaStyle(StrokeStyle);

macro_rules! set {
    ($field:expr, $result:expr) => {
        if let Ok(value) = $result {
            $field = value;
        }
    };
}

impl LuaStyle {
    pub fn lua_clone<'lua>(&self, value: Value<'lua>) -> mlua::Result<Self> {
        let d = match value {
            Value::Nil => self.clone(),
            Value::Table(t) => {
                let mut style = self.clone();

                set!(style.0.width, t.get("width"));

                set!(style.0.cap, t.get("cap").map(|x: String| read_linecap(&x))?);
                set!(
                    style.0.join,
                    t.get("join").map(|x: String| read_linejoin(&x))?
                );

                set!(style.0.miter_limit, t.get("miter_limit"));
                set!(style.0.dash_array, t.get("dash_array"));
                set!(style.0.dash_offset, t.get("dash_offset"));
                style
            }
            _ => {
                return Err(Error::FromLuaConversionError {
                    from: "string",
                    to: "LineJoin",
                    message: None,
                })
            }
        };
        Ok(d)
    }
}

impl UserData for LuaStyle {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("clone", |_, style, arg| style.lua_clone(arg));
    }

    fn add_fields<'lua, F: mlua::prelude::LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, style| Ok(style.0.width));
        fields.add_field_method_get("cap", |_, style| {
            let str = match style.0.cap {
                LineCap::Round => "round",
                LineCap::Square => "square",
                LineCap::Butt => "butt",
            };
            Ok(str)
        });
        fields.add_field_method_get("join", |_, style| {
            let str = match style.0.join {
                LineJoin::Round => "round",
                LineJoin::Miter => "miter",
                LineJoin::Bevel => "bevel",
            };
            Ok(str)
        });
        fields.add_field_method_get("miter_limit", |_, style| Ok(style.0.miter_limit));
        fields.add_field_method_get("dash_array", |_, style| Ok(style.0.dash_array.clone()));
        fields.add_field_method_get("dash_offset", |_, style| Ok(style.0.dash_offset));

        fields.add_field_method_set("width", |_, style, v: Number| {
            style.0.width = v as f32;
            Ok(())
        });
        fields.add_field_method_set("cap", |_, style, v: String| {
            style.0.cap = read_linecap(&v)?;
            Ok(())
        });
        fields.add_field_method_set("join", |_, style, v: String| {
            style.0.join = read_linejoin(&v)?;
            Ok(())
        });
        fields.add_field_method_set("miter_limit", |_, style, v: Number| {
            style.0.miter_limit = v as f32;
            Ok(())
        });
        fields.add_field_method_set("dash_array", |_, style, v: Table| {
            let vec: mlua::Result<Vec<f32>> = v.sequence_values::<f32>().collect();
            style.0.dash_array = vec?;
            Ok(())
        });
        fields.add_field_method_set("dash_offset", |_, style, v: Number| {
            style.0.dash_offset = v as f32;
            Ok(())
        });
    }
}

#[derive(Clone, FromLua)]
struct LuaColor(SolidSource);

impl LuaColor {
    pub fn lua_clone<'lua>(&self, value: Value<'lua>) -> mlua::Result<Self> {
        let d = match value {
            Value::Nil => self.clone(),
            Value::Table(t) => {
                let mut color = self.clone();

                set!(color.0.r, t.get("r"));
                set!(color.0.g, t.get("g"));
                set!(color.0.b, t.get("b"));
                set!(color.0.a, t.get("r"));
                color
            }
            _ => {
                return Err(Error::FromLuaConversionError {
                    from: "string",
                    to: "LineJoin",
                    message: None,
                })
            }
        };
        Ok(d)
    }
}

impl UserData for LuaColor {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("clone", |_, color, arg| color.lua_clone(arg));
    }
    fn add_fields<'lua, F: mlua::prelude::LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("r", |_, color| Ok(color.0.r));
        fields.add_field_method_get("g", |_, color| Ok(color.0.g));
        fields.add_field_method_get("b", |_, color| Ok(color.0.b));
        fields.add_field_method_get("a", |_, color| Ok(color.0.a));

        fields.add_field_method_set("r", |_, color, v: Number| {
            color.0.r = v as u8;
            Ok(())
        });
        fields.add_field_method_set("g", |_, color, v: Number| {
            color.0.g = v as u8;
            Ok(())
        });
        fields.add_field_method_set("b", |_, color, v: Number| {
            color.0.b = v as u8;
            Ok(())
        });
        fields.add_field_method_set("a", |_, color, v: Number| {
            color.0.a = v as u8;
            Ok(())
        });
    }
}

impl Default for LuaColor {
    fn default() -> Self {
        let solid = SolidSource {
            r: 255,
            b: 0,
            g: 0,
            a: 0,
        };
        LuaColor(solid)
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
            let (sender, receiver) = calloop::channel::sync_channel::<Message>(3);
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
                            Message::Drawing(s, pos, draw) => {
                                let draw = draw.into();
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

        methods.add_method(
            "draw",
            |_, cb, (name, table, draw): (String, Table, LuaDraw)| {
                let x = table.get("x")?;
                let y = table.get("y")?;
                cb.sender
                    .send(Message::Drawing(name, (x, y), draw))
                    .unwrap();
                Ok(())
            },
        );
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
        Value::Table(tbl) => {
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

pub fn get_or_create_module<'lua>(lua: &'lua Lua, name: &str) -> mlua::Result<()> {
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let loaded: Table = package.get("loaded")?;

    let module = loaded.get(name)?;
    match module {
        Value::Nil => {
            let table = lua.create_table()?;
            let draw = lua.create_function(|_, value: Value| {
                let d = LuaDraw::default();
                d.lua_clone(value)
            })?;
            let style = lua.create_function(|_, value: Value| {
                let s = LuaStyle::default();
                s.lua_clone(value)
            })?;
            let color = lua.create_function(|_, value: Value| {
                let c = LuaColor::default();
                c.lua_clone(value)
            })?;
            table.set("Draw", draw)?;
            table.set("Style", style)?;
            table.set("Color", color)?;
            loaded.set(name, table)?;
            Ok(())
        }
        Value::Table(_) => Ok(()),
        wat => todo!(),
        // wat => anyhow::bail!(
        //     "cannot register module {} as package.loaded.{} is already set to a value of type {}",
        //     name,
        //     name,
        //     wat.type_name()
        // ),
    }
}

pub fn make_lua_context(config_file: &Path) -> anyhow::Result<()> {
    let lua = Rc::new(Lua::new());

    get_or_create_runtime(lua.clone(), "sketchover")?;
    get_or_create_module(&lua, "sketchover.draw")?;
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
