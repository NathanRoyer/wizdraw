#version 100
precision highp float;

uniform vec2 offset;
uniform float height;
uniform sampler2D opacity;

uniform vec2 bmp_size;
uniform vec2 bmp_tile_offset;
uniform sampler2D bmp_tile;

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
        vec2 top_left = param_1.xy; // 600, 400
        float scale = param_1.z; // 1.0
        bool repeat = param_1.w != 0.0; // true

        vec2 scaled_size = bmp_size * scale; // 316, 316

        vec2 offset = gl_FragCoord.xy - top_left; // 10, 10
        if (repeat) offset = mod(offset, scaled_size);

        offset = offset / scale; // 10, 10
        // offset = offset - bmp_tile_offset; // 60, 60
        offset = offset / bmp_size;

        bool invalid_x = 0.0 > offset.x || offset.x > 1.0;
        bool invalid_y = 0.0 > offset.y || offset.y > 1.0;

        if (invalid_x || invalid_y) {
            // out of bounds
            discard;
        }

        gl_FragColor = texture2D(bmp_tile, offset);
        // gl_FragColor = vec4(offset, 0.5, 1);

    } else if (mode == 4) {
        // quad bitmap

        gl_FragColor = rainbow(gl_FragCoord.xy);

    } else {
        // debug / gradient

        gl_FragColor = rainbow(gl_FragCoord.xy);

    }
}
