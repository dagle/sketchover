local sketchover = require("sketchover")
local save = false

local tools = {
	"pen",
	"line",
	"rect",
}

local idx = 1
local current_tool = tools[idx]

local Pen = {}
Pen.__index = Pen

local function extend(orig, override)
	local copy

	if type(orig) == "table" then
		copy = {}

		-- Copy original table
		for orig_key, orig_value in pairs(orig) do
			copy[orig_key] = extend(orig_value, override and override[orig_key])
		end

		-- Add missing keys from override
		if override then
			for override_key, override_value in pairs(override) do
				if copy[override_key] == nil then
					copy[override_key] = extend(override_value)
				end
			end
		end

		setmetatable(copy, extend(getmetatable(orig), getmetatable(override)))
	else
		copy = override or orig
	end

	return copy
end

function Pen:new(override, base)
	local pen = {}
	pen.draw = extend(base or {
		color = {
			r = 0,
			g = 255,
			b = 0,
			a = 255,
		},

		style = {
			width = 8,
			cap = "round",
			join = "miter",
			miter_limit = 10,
			dash_array = {},
			dash_offset = 0,
		},
	}, override)
	return setmetatable(pen, Pen)
end

function Pen:clone(override)
	return Pen:new(override, self.draw)
end

function Pen:commit()
	return self.draw
end

local p = Pen:new()

-- lets define a function swap between colors
function Pen:next()
	if self.draw.color.r == 255 then
		self.draw.color.r = 0
		self.draw.color.g = 255
	elseif self.draw.color.g == 255 then
		self.draw.color.g = 0
		self.draw.color.b = 255
	else
		self.draw.color.b = 0
		self.draw.color.r = 255
	end
end

local outputs = {}

sketchover.init(function(cb)
	-- lets start
	-- cb:pause()
end)

sketchover.new_output(function(cb, info)
	table.insert(outputs, info)

	-- try to restore the output
	-- info:restore(true)
end)

sketchover.destroy_output(function(cb, id)
	for i, out in ipairs(outputs) do
		if out.id == id then
			table.remove(outputs, i)
			return
		end
	end
end)

local function empty_match(var)
	if type(var) ~= "table" then
		return true
	end

	return next(var) == nil
end

--- Search our outputs for one that matches all of our fields
--- @param fields table
local function find_output(fields)
	if empty_match(fields) then
		return nil
	end

	for i, output in ipairs(outputs) do
		local matched = true

		for key, value in pairs(fields) do
			if output[key] ~= value then
				matched = false
				break
			end
		end
		if matched then
			return i, output
		end
	end
end

sketchover.mousepress(function(cb, event, press)
	if press then
		if event.button == "left" then
			cb:draw(current_tool, event.pos, p:commit())
		end
		p:next()
	else
		cb:stop_draw()
	end
end)

sketchover.keypress(function(cb, event, press)
	if not press and event.key == "XK_m" then
		cb:stop_draw()
	end

	if press then
		if event.key == "XK_m" then
			cb:draw(current_tool, event.pos, p:commit())
		end

		if event.key == "XK_q" then
			cb:quit()
		end

		-- by default, commands take the current screen
		-- when doing an undo.
		if event.key == "XK_u" then
			cb:undo()
		end

		if event.key == "XK_U" then
			local _, output = find_output({ name = "eDP-1" })
			if output then
				-- We now undo on a specific screen
				cb:undo(output.id)
			else
				-- if that screen isn't find, lets fall back
				cb:undo()
			end
		end
		if event.key == "XK_p" then
			cb:pause()
		end
		if event.key == "XK_P" then
			cb:unpause()
		end
		if event.key == "XK_c" then
			cb:clear()
		end

		if event.key == "XK_n" then
			idx = idx % #tools + 1
			current_tool = tools[idx]
		end

		if event.key == "XK_N" then
			if idx == 1 then
				idx = #tools
			else
				idx = idx - 1
			end
			current_tool = tools[idx]
		end

		if event.key == "XK_s" then
			-- this saves the current screen
			cb:save()
		end
		if event.key == "XK_S" then
			-- or we can save all screens
			for _, output in ipairs(outputs) do
				cb:save(output.id)
			end
		end
		if event.key == "XK_s" and event.modifiers.ctrl then
			-- toggle if we should save on exit
			save = not save
		end
	end
end)

-- sketchover starts to run, the object is now locked until run returns.
sketchover:run()

-- if save then
-- 	-- save all screens after this.
-- 	sketchover:save()
-- end
