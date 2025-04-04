#version 100
precision highp float;
const float epsilon = 0.0001;

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
    int i = int(mod(p, 8.0));
    return rainbow[i] / 255.0;
}

vec4 sample_tile(vec2 offset) {
    bool invalid_x = 0.0 > offset.x || offset.x > bmp_size.x;
    bool invalid_y = 0.0 > offset.y || offset.y > bmp_size.y;

    if (invalid_x || invalid_y) {
        // out of bitmap bounds
        discard;
    }

    offset = offset - bmp_tile_offset;
    invalid_x = 0.0 > offset.x || offset.x > 255.0;
    invalid_y = 0.0 > offset.y || offset.y > 255.0;

    if (invalid_x || invalid_y) {
        // out of bitmap tile bounds
        discard;
    }

    offset = offset / 255.0;
    return texture2D(bmp_tile, offset);
}

float wedge(vec2 a, vec2 b) {
    return a.x * b.y - a.y * b.x;
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

        gl_FragColor = param_1 / 255.0;

    } else if (mode == 3) {
        // bitmap

        // parameters
        vec2 top_left = param_1.xy;
        float scale = param_1.z;
        bool repeat = param_1.w != 0.0;

        vec2 scaled_size = bmp_size * scale;

        vec2 offset = gl_FragCoord.xy - top_left;
        if (repeat) offset = mod(offset, scaled_size);

        offset = offset / scale;

        gl_FragColor = sample_tile(offset);
        // gl_FragColor = vec4(offset, 0.5, 1);

    } else if (mode == 4) {
        // quad bitmap

        // corners of the quad
        vec2 tl = param_1.xy;
        vec2 bl = param_1.zw;
        vec2 tr = param_2.xy;
        vec2 br = param_2.zw;
        vec2 pt = gl_FragCoord.xy;

        vec2 e = tr - tl;
        vec2 f = bl - tl;
        vec2 g = tl - tr + br - bl;
        vec2 h = pt - tl;

        float k2 = wedge(g, f);
        float k1 = wedge(e, f) + wedge(h, g);
        float k0 = wedge(h, e);

        float u, v;
        if (abs(k2) < epsilon) {
            // if edges are parallel, this is a linear equation

            u = (h.x * k1 + f.x * k0) / (e.x * k1 - g.x * k0);
            v = -k0 / k1;

        } else {
            // otherwise, it's a quadratic
            float d = k1 * k1 - 4.0 * k0 * k2;

            if (d < 0.0) {
                discard;
            }

            float w = sqrt(d);

            float ik2 = 0.5 / k2;
            v = (-k1 - w) * ik2;
            u = (h.x - f.x * v) / (e.x + g.x * v);

            if (u < 0.0 || u > 1.0 || v < 0.0 || v > 1.0) {
                v = (-k1 + w) * ik2;
                u = (h.x - f.x * v) / (e.x + g.x * v);
            }
        }

        // gl_FragColor = vec4(offset, 0.5, 1);
        vec2 offset = vec2(u, v) * bmp_size;
        gl_FragColor = sample_tile(offset);

    } else {
        // debug / gradient
        gl_FragColor = rainbow(gl_FragCoord.xy);
    }
}
