#version 300 es

precision highp float;
#define INF_LOOP while (true)

uniform float straight_threshold;
uniform float aabb_safe_margin;
uniform int show_holes;

uniform float path[16];
uniform int path_len;

struct bounding_box_t {
    vec2 min;
    vec2 max;
};

struct cubic_bezier_t {
    vec2 c1;
    vec2 c2;
    vec2 c3;
    vec2 c4;
};

struct subcurves_t {
    cubic_bezier_t a;
    cubic_bezier_t b;
};

bool close_enough(vec2 c1, vec2 c4, vec2 p) {
   vec2 v1 = c4 - c1;
   vec2 v2 = c1 - p;
   vec2 v3 = vec2(v1.y,-v1.x);
   return abs(dot(v2,normalize(v3))) < straight_threshold;
}

bool is_curve_straight(cubic_bezier_t curve) {
    return close_enough(curve.c1, curve.c4, curve.c2)
        && close_enough(curve.c1, curve.c4, curve.c3);
}

bool is_p_in_aabb(vec2 p, bounding_box_t bb) {
    return (bb.min.x - aabb_safe_margin) <= p.x
        && (bb.min.y - aabb_safe_margin) <= p.y
        && (bb.max.x + aabb_safe_margin) >= p.x
        && (bb.max.y + aabb_safe_margin) >= p.y;
}

// Computes a winding number addition based on [S -> E] segment and point P
int use_segment_for_pip(vec2 p, vec2 s, vec2 e) {
    vec2 v1 = p - s;
    vec2 v2 = e - s;

    bool b1 = s.y <= p.y;
    bool b2 = e.y > p.y;
    bool b3 = (v1.x * v2.y) > (v1.y * v2.x);

    bool dec = ( b1) && ( b2) && ( b3);
    bool inc = (!b1) && (!b2) && (!b3);

    return int(inc) - int(dec);
}

// from a to b
vec2 travel(vec2 a, vec2 b, float t) {
    float x = a.x + (b.x - a.x) * t;
    float y = a.y + (b.y - a.y) * t;
    return vec2(x, y);
}

subcurves_t curve_split(cubic_bezier_t curve, float t) {
    vec2 side1 = travel(curve.c1, curve.c2, t);
    vec2 side2 = travel(curve.c2, curve.c3, t);
    vec2 side3 = travel(curve.c3, curve.c4, t);

    vec2 diag1 = travel(side1, side2, t);
    vec2 diag2 = travel(side2, side3, t);

    vec2 split_point = travel(diag1, diag2, t);

    cubic_bezier_t first_half = cubic_bezier_t(
        curve.c1,
        side1,
        diag1,
        split_point
    );

    cubic_bezier_t second_half = cubic_bezier_t(
        split_point,
        diag2,
        side3,
        curve.c4
    );

    return subcurves_t(
        first_half,
        second_half
    );
}

bounding_box_t curve_aabb(cubic_bezier_t curve) {
    float min_x = min(min(curve.c1.x, curve.c2.x), min(curve.c3.x, curve.c4.x));
    float min_y = min(min(curve.c1.y, curve.c2.y), min(curve.c3.y, curve.c4.y));

    float max_x = max(max(curve.c1.x, curve.c2.x), max(curve.c3.x, curve.c4.x));
    float max_y = max(max(curve.c1.y, curve.c2.y), max(curve.c3.y, curve.c4.y));

    return bounding_box_t(
        vec2(min_x, min_y),
        vec2(max_x, max_y)
    );
}

cubic_bezier_t read_curve(int index) {
    int offset = index * 8;

    vec2 c1 = vec2(path[offset + 0], path[offset + 1]);
    vec2 c2 = vec2(path[offset + 2], path[offset + 3]);
    vec2 c3 = vec2(path[offset + 4], path[offset + 5]);
    vec2 c4 = vec2(path[offset + 6], path[offset + 7]);

    return cubic_bezier_t(c1, c2, c3, c4);
}

bool subpixel_is_in_path(vec2 pixel) {
    cubic_bezier_t rem_sc = read_curve(0);
    int path_index = 0;
    int winding_number = 0;
    float trial = 1.0;

    INF_LOOP {
        subcurves_t split_result = curve_split(rem_sc, trial);
        cubic_bezier_t trial_sc = split_result.a;
        cubic_bezier_t future_sc = split_result.b;

        bounding_box_t trial_aabb = curve_aabb(trial_sc);
        bool p_out_of_trial_aabb = !is_p_in_aabb(pixel, trial_aabb);
        bool use_as_is = p_out_of_trial_aabb || is_curve_straight(trial_sc);

        if (use_as_is) {

            winding_number += use_segment_for_pip(pixel, trial_sc.c1, trial_sc.c4);

            // did we complete curve curve?
            if (trial == 1.0) {
                path_index += 1;
                if (path_index < path_len) {
                    rem_sc = read_curve(path_index);
                } else {
                    break;
                }
            } else {
                rem_sc = future_sc;
                trial = 1.0;
            }

        } else {
            trial *= 0.5;
        }
    }

    if (show_holes != 0) {
        winding_number = int(mod(float(winding_number), 2.0));
    }

    return path_index >= path_len && winding_number != 0;
}

void main() {
    if (subpixel_is_in_path(gl_FragCoord.xy)) {
        gl_FragColor = vec4(1, 1, 0, 1);
    } else {
        gl_FragColor = vec4(0);
    }
}
