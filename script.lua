function hsv_to_rgb( hue, saturation, value )
	-- Returns the RGB equivalent of the given HSV-defined color
	-- (adapted from some code found around the web)

	-- If it's achromatic, just return the value
	if saturation == 0 then
		return value;
	end;

	-- Get the hue sector
	local hue_sector = math.floor( hue / 60 );
	local hue_sector_offset = ( hue / 60 ) - hue_sector;

	local p = value * ( 1 - saturation );
	local q = value * ( 1 - saturation * hue_sector_offset );
	local t = value * ( 1 - saturation * ( 1 - hue_sector_offset ) );

	if hue_sector == 0 then
		return value, t, p;
	elseif hue_sector == 1 then
		return q, value, p;
	elseif hue_sector == 2 then
		return p, value, t;
	elseif hue_sector == 3 then
		return p, q, value;
	elseif hue_sector == 4 then
		return t, p, value;
	elseif hue_sector == 5 then
		return value, p, q;
	end;
end;

local hue = 269
local saturation = 1.0
local value = 0.8

local hue_ram = hue

local increment = 0.01

function temps()
    local handle = io.popen("sensors -j")
    local result = handle:read("*a")
    handle:close()

    return load("return " .. string.gsub(result, [["([%w%s-_]+)":]], [[["%1"] = ]]))()
end

function temp_scaled()
    local temp = temps()["k10temp-pci-00cb"].Tdie.temp1_input
    return math.min(30, math.max(0, temp - 40)) / 30
end

local t = 0
local t_last = 0
function tick()
    local r, g, b = hsv_to_rgb(hue + math.cos(t) * 10, saturation, value + math.sin(t/4) * 0.2)
    local r2, g2, b2 = hsv_to_rgb(hue + math.sin(t) * 10, saturation, value + math.sin(t/4) * 0.2)

    local c = {}
    for i = 1, controllers["MB"] do
        table.insert(c, r * 255)
        table.insert(c, g * 255)
        table.insert(c, b * 255)
    end
    set_colours("MB", c)

    c = {}
    for i = 1, controllers["RAM77"] do
        if i == 1 or i == math.ceil(controllers["RAM77"]/2) or i == controllers["RAM77"] then
            table.insert(c, r * 255)
            table.insert(c, g * 255)
            table.insert(c, b * 255)
        else
            table.insert(c, r2 * 255)
            table.insert(c, g2 * 255)
            table.insert(c, b2 * 255)
        end
    end
    set_colours("RAM77", c)
--[[
    local c = {}
    local r, g, b = hsv_to_rgb(hue_ram, saturation, value)
    local tx = math.abs(math.sin(t)) * 5

    if t - t_last > math.pi then
        hue_ram = (hue_ram + 10) % 360
        t_last = t
    end

    for i = 1, 5 do
        local adjusted_i = math.abs(i - 3) * 2.5

        if adjusted_i <= tx then
            table.insert(c, r * 255)
            table.insert(c, g * 255)
            table.insert(c, b * 255)
        else
            local sf = (1 - math.max(0, math.min(1, adjusted_i - tx)))
            table.insert(c, r * 255 * sf)
            table.insert(c, g * 255 * sf)
            table.insert(c, b * 255 * sf)
        end
    end
    set_colours("RAM77", c)
]]--
    t = t + 0.03
end
