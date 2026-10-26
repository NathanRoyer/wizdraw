#version 100
precision highp float;

const float TEXTURE_SIZE = 64.0;
const float PI = 3.1415926538;
const float Y_BIAS = 0.11;

uniform sampler2D prev_iteration;
uniform float height;
uniform vec2 offset;
uniform int blank;

// [0.0, a.x, a.y, b.x, b.y] => segment
// [radius, c.x, c.y, min_magic, max_magic] => non-wrapping arc
// [-radius, c.x, c.y, min_magic, max_magic] => wrapping arc
uniform float input_buf[80];
uniform int input_len;

bool segment(vec2 pos, float a_x, float a_y, float b_x, float b_y) {
    if (a_y == b_y) {
        return false;
    }

    float t = (a_y - pos.y) / (a_y - b_y);

    if (t < 0.0 || t > 1.0) {
        return false;
    }

    float offset = a_x + (b_x - a_x) * t;
    return pos.x < offset;
}

vec2 cos_range(float min_cos, float max_cos, bool min_neg, bool max_neg, bool row_pos) {
    float fake = 2.0;
    float cos_a;
    float cos_b;

    /**/ if ( min_neg &&  max_neg &&  row_pos) { cos_a = fake;    cos_b = fake;    }
    else if ( min_neg &&  max_neg && !row_pos) { cos_a = min_cos; cos_b = max_cos; }
    else if ( min_neg && !max_neg &&  row_pos) { cos_a = 1.0;     cos_b = max_cos; }
    else if ( min_neg && !max_neg && !row_pos) { cos_a = min_cos; cos_b = 1.0;     }
    else if (!min_neg && !max_neg &&  row_pos) { cos_a = min_cos; cos_b = max_cos; }
    else if (!min_neg && !max_neg && !row_pos) { cos_a = fake;    cos_b = fake;    }

    bool sorted = cos_a <= cos_b;
    float lil_cos = sorted ? cos_a : cos_b;
    float big_cos = sorted ? cos_b : cos_a;

    return vec2(lil_cos, big_cos);
}

bool arc(vec2 pos, float c_x, float c_y, vec2 cos, bvec2 neg, float radius, bool wraps) {
    float sin = (pos.y - c_y) / radius;

    if (sin < -1.0 || sin > 1.0) {
        return false;
    }

    vec2 range = cos_range(cos.x, cos.y, neg.x, neg.y, sin < 0.0);

    float r_cos = sqrt(1.0 - sin * sin);
    float l_cos = -r_cos;

    bool r_col = range.x <= r_cos && r_cos <= range.y;
    bool l_col = range.x <= l_cos && l_cos <= range.y;

    if (wraps) {
        r_col = !r_col;
        l_col = !l_col;
    }

    bool toggled = false;

    if (l_col) {
        float offset = c_x + l_cos * radius;
        toggled = pos.x < offset;
    }

    if (r_col) {
        float offset = c_x + r_cos * radius;
        if (pos.x < offset) toggled = !toggled;
    }

    return toggled;
}

void main() {
    vec2 win_pos = gl_FragCoord.xy;
    vec2 tex_pos = win_pos / TEXTURE_SIZE;
    vec4 rgba = vec4(0);

    if (blank == 0) rgba = texture2D(prev_iteration, tex_pos);

    // one bit winding number blank
    bool wind_num = rgba.x != 0.0;

    win_pos.y += offset.y;
    win_pos.x += offset.x;

    // texture vertical flip
    win_pos.y = height - win_pos.y + Y_BIAS;

    for (int i = 0; i < 80; i += 5) {
        if (i >= input_len) break;

        float x = input_buf[i + 0];
        float a = input_buf[i + 1];
        float b = input_buf[i + 2];
        float c = input_buf[i + 3];
        float d = input_buf[i + 4];
        bool toggled;

        if (x == 0.0) {
            toggled = segment(win_pos, a, b, c, d);
        } else {
            float radius = abs(x);
            bool wraps = x < 0.0;
            bool min_neg = c > 2.5;
            bool max_neg = d > 2.5;
            if (min_neg) c -= 4.0;
            if (max_neg) d -= 4.0;
            vec2 cos = vec2(c, d);
            bvec2 neg = bvec2(min_neg, max_neg);

            toggled = arc(win_pos, a, b, cos, neg, radius, wraps);
        }

        if (toggled) wind_num = !wind_num;
    }

    float r = wind_num ? 1.0 : 0.0;
    gl_FragColor = vec4(r, 0.5, 0.5, 1);
}
