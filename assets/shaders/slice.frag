#version 150 core
in vec3 v_gpos;
out vec4 o_Color;
uniform float u_color_scale;
uniform float u_trans_size;
uniform float u_trans_num;
uniform sampler1D u_color_map;
uniform sampler1D u_trans_pos;
uniform sampler1D u_trans_pos_256;
uniform sampler1D u_trans_pos_sub;
uniform sampler1D u_trans_phase;
const float PI = 3.141592653589793;
const float WAVE_LENGTH = 8.5;
const float WAVE_NUM = 2.0*PI/WAVE_LENGTH;
vec4 coloring(float t)
{
  return texture(u_color_map, clamp(t * u_color_scale, 0.0, 0.99));
}
void main() {
    float re = 0.0;
    float im = 0.0;
    for(float idx = 0.0; idx < 65536.0; idx++){
        if (idx >= u_trans_num) break;
        vec3 t = texture(u_trans_pos, (idx+0.5) / u_trans_num).xyz;
        vec3 t_256 = texture(u_trans_pos_256, (idx+0.5) / u_trans_num).xyz;
        vec3 t_sub = texture(u_trans_pos_sub, (idx+0.5) / u_trans_num).xyz;
        vec3 tr = floor(255.0 * t);
        vec3 tr_256 = 256.0 * floor(255.0 * t_256);
        vec3 tp = u_trans_size * (tr + tr_256 + t_sub);
        float p = 2.0*PI*texture(u_trans_phase, (idx+0.5) / u_trans_num).x;
        float d = length(v_gpos - tp);
        im += cos(p - WAVE_NUM*d) / d;
        re += sin(p - WAVE_NUM*d) / d;
    }
    float c = sqrt(re*re + im*im);
    o_Color = coloring(c);
}