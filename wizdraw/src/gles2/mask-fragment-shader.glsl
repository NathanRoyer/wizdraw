#version 100
precision highp float;
const float TEXTURE_SIZE = 256.0;
const float PI = 3.1415926538;

uniform sampler2D prev_iteration;
uniform float height;
uniform vec2 offset;
uniform int blank;

// [0.0, a.x, a.y, b.x, b.y] => segment
// [radius, c.x, c.y, start_angle, stop_angle] => counterclockwise arc
// [-radius, c.x, c.y, start_angle, stop_angle] => clockwise arc
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

bool covers_angle(float angle, bool wraps, float min_angle, float max_angle) {
    if (wraps) {
        bool r1 = -PI <= angle && angle <= min_angle;
        bool r2 = max_angle <= angle && angle <= PI;
        return r1 || r2;
    } else {
        return min_angle <= angle && angle <= max_angle;
    }
}

bool arc(vec2 pos, float c_x, float c_y, float a1, float a2, float radius, bool clockwise) {
    float sin = (pos.y - c_y) / radius;

    if (sin < -1.0 || sin > 1.0) {
        return false;
    }

    // negation: screen coordinates
    float r_angle = asin(-sin); // (-HALF_PI..=HALF_PI)
    float l_angle = (r_angle < 0.0) ? (-PI - r_angle) : (PI - r_angle);

    bool sorted = a1 < a2;
    bool wraps = clockwise == sorted;
    float min_angle = sorted ? a1 : a2;
    float max_angle = sorted ? a2 : a1;

    bool toggled = false;

    if (covers_angle(l_angle, wraps, min_angle, max_angle)) {
        float l_cos = cos(l_angle);
        float offset = c_x + l_cos * radius;
        toggled = pos.x < offset;
    }

    if (covers_angle(r_angle, wraps, min_angle, max_angle)) {
        float r_cos = cos(r_angle);
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
    win_pos.y = height - win_pos.y;

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
            bool clockwise = x < 0.0;
            toggled = arc(win_pos, a, b, c, d, radius, clockwise);
        }

        if (toggled) wind_num = !wind_num;
    }

    float r = wind_num ? 1.0 : 0.0;
    gl_FragColor = vec4(r, 0.5, 0.5, 1);
}
