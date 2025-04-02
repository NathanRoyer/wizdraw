#version 100
precision highp float;

uniform vec2 offset;
uniform float height;
uniform sampler2D opacity;

void main() {
    vec2 inverted = vec2(gl_FragCoord.x, gl_FragCoord.y);
    vec2 pos = (inverted - offset) / vec2(256.0, 256.0);

    vec4 rgba = vec4(0);
    if (pos.x > 0.0 && pos.y > 0.0 && pos.x < 1.0 && pos.y < 1.0) {
        rgba = texture2D(opacity, pos);
    }

    if (rgba.x == 1.0) {
        // it's in
        gl_FragColor = vec4(pos, 0.5, 1);
    } else {
        // it's out
        discard;
        // gl_FragColor = vec4(0);
    }
}
