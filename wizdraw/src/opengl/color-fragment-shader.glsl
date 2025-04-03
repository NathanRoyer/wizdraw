#version 100
precision highp float;

uniform vec2 offset;
uniform float height;
uniform sampler2D opacity;

uniform vec2 bmp_size;
uniform vec2 bmp_tile_offset;
// uniform sampler2D bmp_tile;

// 0 = solid color
// 1 = gradient (todo)
// 2 = debug
// 3 = bitmap
// 4 = quad bitmap
uniform int mode;

// mode 0 = R, G, B, A
// mode 3 = x, y, scale, repeat
// mode 4 = tl.x, tl.y, bl.x, bl.y
uniform vec4 param_1;

// mode 4 = tr.x, tr.y, br.x, br.y
uniform vec4 param_2;

vec4 rainbow(vec2 point) {
    vec4 rainbow[8];

    rainbow[0] = vec4(255,   0,   0, 255);
    rainbow[1] = vec4(255, 127,   0, 255);
    rainbow[2] = vec4(255, 255,   0, 255);
    rainbow[3] = vec4(  0, 255,   0, 255);
    rainbow[4] = vec4(  0,   0, 255, 255);
    rainbow[5] = vec4( 75,   0, 130, 255);
    rainbow[6] = vec4(148,   0, 211, 255);
    rainbow[7] = vec4(255, 255, 255,   0);

    float p = (point.x + point.y) / 16.0;
    return rainbow[int(mod(p, 8.0))];
}

void main() {
    vec2 pos = (gl_FragCoord.xy - offset) / vec2(256.0, 256.0);

    vec4 rgba = vec4(0);
    if (pos.x > 0.0 && pos.y > 0.0 && pos.x < 1.0 && pos.y < 1.0) {
        rgba = texture2D(opacity, pos);
    }

    if (rgba.x != 1.0) {
        // it's out
        discard;
    }

    /*__*/ if (mode == 0) {
        // solid color

        gl_FragColor = param_1;

    } else if (mode == 3) {
        // bitmap

        // parameters
        vec2 top_left = param_1.xy;
        float scale = param_1.z;
        bool repeat = param_1.w != 0.0;

        vec2 scaled_size = bmp_size * scale;

        vec2 offset = gl_FragCoord.xy - top_left;
        if (repeat) offset = mod(offset, scaled_size);

        float min_x = bmp_tile_offset.x * scale;
        float min_y = bmp_tile_offset.y * scale;
        float max_x = min_x + (256.0 * scale);
        float max_y = min_y + (256.0 * scale);

        bool invalid_x = min_x > offset.x || offset.x > max_x;
        bool invalid_y = min_y > offset.y || offset.y > max_y;

        if (invalid_x || invalid_y) {
            // out of bounds
            discard;
        }

        gl_FragColor = texture2D(opacity, offset / scale);

    } else if (mode == 4) {
        // quad bitmap

        gl_FragColor = rainbow(gl_FragCoord.xy);

    } else {
        // debug / gradient

        gl_FragColor = rainbow(gl_FragCoord.xy);

    }
}
