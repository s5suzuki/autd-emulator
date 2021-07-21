#version 150 core

in vec3 v_gpos;
out vec4 o_Color;

uniform float u_wavenum;
uniform float u_color_scale;
uniform float u_trans_num;
uniform sampler1D u_color_map;
uniform sampler1D u_trans_pos;
uniform sampler1D u_trans_drive;

const float PI = 3.141592653589793;

vec4 coloring(float t)
{
  return texture(u_color_map, clamp(t, 0.0, 1.0));
}

void main() {
    float re = 0.0;
    float im = 0.0;
    for(float idx = 0.0; idx < 65536.0; idx++){
        if (idx >= u_trans_num) break;
        vec3 tp = texture(u_trans_pos, (idx+0.5) / u_trans_num).xyz;
        float d = length(v_gpos - tp);
        vec2 p_amp = texture(u_trans_drive, (idx+0.5) / u_trans_num).xy;
        float p = -2.0*PI*p_amp.x;
        float amp = p_amp.y / d;
        im += amp * cos(p - u_wavenum*d);
        re += amp * sin(p - u_wavenum*d);
    }
    float c = sqrt(re*re+im*im);
    o_Color = coloring(c/u_color_scale);
}
