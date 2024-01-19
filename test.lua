local sketchover = require("sketchover")

local save = false

local current_tool = "pen"
local idx = 1

local tools = {
	"pen",
	"line",
	"box",
}

sketchover.on("init", function()
	sketchover.signal("sigtstp", function()
		sketchover.set_passthrough(true)
	end)

	sketchover.set_fg({
		r = 8,
		g = 8,
		b = 8,
		a = 0,
	})
end)

sketchover.on("new-output", function(output)
	local name = output:name()

	if sketchover.savefile_exist() then
		output:load(name)
	end
	sketchover.push_put(output)
end)

-- we don't care atm, but you could do stuff like save the output etc
-- sketchover.on("remove-output", function() end)

sketchover.on("keypress", function(event)
	if event.key == "q" then
		sketchover.quit()
	end
	if event.key == "s" then
		save = true
	end

	-- select the next tool
	if event.key == "n" then
		idx = idx % #tools
		idx = idx + 1
		current_tool = tools[idx]
	end

	-- select the prev tool
	if event.key == "n" and event.modifier.shift then
		if idx == 1 then
			idx = #tools
		else
			idx = idx + 1
		end
		current_tool = tools[idx]
	end

	-- Set the tool to line and set the tool index
	-- to be line
	if event.key == "l" then
		current_tool = "line"
		idx = 2
	end

	if event.key == "p" then
		sketchover.pause()
	end

	if event.key == "u" then
		sketchover.undo()
	end
end)

sketchover.on("mousepress", function(event)
	if event.button == "left" then
		sketchover.start_drawing(current_tool)
	end
end)

sketchover.on("shutdown", function()
	local savefile = sketchover.savefile()
	if save then
		sketchover.save(savefile)
	else
		-- rm the sketchover
	end
end)
