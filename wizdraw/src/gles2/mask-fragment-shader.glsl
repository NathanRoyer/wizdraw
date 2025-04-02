#version 100
precision highp float;
const float TEXTURE_SIZE = 256.0;
const float epsilon = 0.0001;
const float straight_threshold = 0.8;

uniform sampler2D prev_iteration;
uniform float height;
uniform vec2 offset;
uniform int init;

// [c1x, c1y, c2x, c2y, c3x, c3y, c4x, c4y]
uniform float input_curve[8];

bool use_segment_for_pip(vec2 p, vec2 s, vec2 e) {
    vec2 v1 = p - s;
    vec2 v2 = e - s;
    float d = v1.x * v2.y - v1.y * v2.x;

    bool b1 = s.y <= p.y;
    bool b2 = e.y > p.y;
    bool b3 = d > epsilon;

    bool dec = ( b1) && ( b2) && ( b3);
    bool inc = (!b1) && (!b2) && (!b3);

    return (int(inc) - int(dec)) != 0;
}

bool is_curve_straight(vec2 curve[4]) {
    // https://en.wikipedia.org/wiki/Distance_from_a_point_to_a_line#Line_defined_by_two_points

    vec2 c1_to_c4 = curve[3] - curve[0];
    float dot_c1_to_c4 = inversesqrt(c1_to_c4.x * c1_to_c4.x + c1_to_c4.y * c1_to_c4.y);

    vec2 c2_to_c1 = curve[0] - curve[1];
    float a_c2 = c1_to_c4.x * c2_to_c1.y;
    float b_c2 = c1_to_c4.y * c2_to_c1.x;
    float d_c2 = abs(a_c2 - b_c2) * dot_c1_to_c4;

    vec2 c3_to_c1 = curve[0] - curve[2];
    float a_c3 = c1_to_c4.x * c3_to_c1.y;
    float b_c3 = c1_to_c4.y * c3_to_c1.x;
    float d_c3 = abs(a_c3 - b_c3) * dot_c1_to_c4;

    // distance from point to projected point
    bool c2_close_enough = d_c2 < straight_threshold;
    bool c3_close_enough = d_c3 < straight_threshold;

    return c2_close_enough && c3_close_enough;
}

vec2 travel(vec2 src, vec2 dst, float t) {
    return src + (dst - src) * t;
}

void split_curve(in vec2 curve[4], in float t, out vec2 trial_sc[4], out vec2 future_sc[4]) {
    vec2 side1 = travel(curve[0], curve[1], t);
    vec2 side2 = travel(curve[1], curve[2], t);
    vec2 side3 = travel(curve[2], curve[3], t);

    vec2 diag1 = travel(side1, side2, t);
    vec2 diag2 = travel(side2, side3, t);

    vec2 split_point = travel(diag1, diag2, t);

    trial_sc[0] = curve[0];
    trial_sc[1] = side1;
    trial_sc[2] = diag1;
    trial_sc[3] = split_point;

    future_sc[0] = split_point;
    future_sc[1] = diag2;
    future_sc[2] = side3;
    future_sc[3] = curve[3];
}

bool aabb_overlap(vec2 pos, vec2 curve[4]) {
    vec2 min_c12 = min(curve[0], curve[1]);
    vec2 max_c12 = max(curve[0], curve[1]);
    vec2 min_c34 = min(curve[2], curve[3]);
    vec2 max_c34 = max(curve[2], curve[3]);

    vec2 min = min(min_c12, min_c34);
    vec2 max = max(max_c12, max_c34);

    bool x_overlap = min.x <= pos.x && pos.x <= max.x;
    bool y_overlap = min.y <= pos.y && pos.y <= max.y;

    return x_overlap && y_overlap;
}

void main() {
    vec2 win_pos = gl_FragCoord.xy;
    vec2 tex_pos = win_pos / TEXTURE_SIZE;
    vec4 rgba = vec4(0);

    if (init == 0) rgba = texture2D(prev_iteration, tex_pos);

    // one bit winding number init
    bool wind_num = rgba.x != 0.0;

    win_pos.y += offset.y;
    win_pos.x += offset.x;

    // texture vertical flip
    win_pos.y = height - win_pos.y;

    vec2 curve[4];
    vec2 future_sc[4];
    vec2 trial_sc[4];

    curve[0] = vec2(input_curve[0], input_curve[1]);
    curve[1] = vec2(input_curve[2], input_curve[3]);
    curve[2] = vec2(input_curve[4], input_curve[5]);
    curve[3] = vec2(input_curve[6], input_curve[7]);

    float trial = 1.0;

    for (int i = 0; i < 1024; i++) {
        split_curve(curve, trial, trial_sc, future_sc);
        bool no_overlap = !aabb_overlap(win_pos, trial_sc);
        bool use_as_is = no_overlap || is_curve_straight(trial_sc);

        if (use_as_is) {
            vec2 c1 = trial_sc[0];
            vec2 c4 = trial_sc[3];
            if (use_segment_for_pip(win_pos, c1, c4)) {
                wind_num = !wind_num;
            }

            // did we complete this curve?
            if (trial == 1.0) {
                break;
            }

            curve[0] = future_sc[0];
            curve[1] = future_sc[1];
            curve[2] = future_sc[2];
            curve[3] = future_sc[3];
            trial = 1.0;
        } else {
            trial *= 0.5;
        }
    }

    float r = wind_num ? 1.0 : 0.0;
    gl_FragColor = vec4(r, 0.5, 0.5, 1);
}
