#version 100
precision highp float;
attribute vec4 a_position;
void main() {
    gl_Position = a_position;
}
