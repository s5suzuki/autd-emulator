#version 150 core

out vec4 o_Color;
uniform vec4 i_Color;

void main() {
    o_Color = i_Color;
}
