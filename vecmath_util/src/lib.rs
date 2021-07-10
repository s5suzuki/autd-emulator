pub type Vector3 = vecmath::Vector3<f32>;
pub type Vector4 = vecmath::Vector4<f32>;
pub type Matrix4 = vecmath::Matrix4<f32>;
pub type Quaterion = quaternion::Quaternion<f32>;

pub fn is_zero<T: std::cmp::PartialEq<T> + num_traits::Zero>(vec: &[T]) -> bool {
    for v in vec {
        if !v.is_zero() {
            return false;
        }
    }
    true
}

pub fn vec3_map<F, I, O>(v: [I; 3], func: F) -> [O; 3]
where
    I: Copy,
    F: Fn(I) -> O,
{
    [func(v[0]), func(v[1]), func(v[2])]
}

pub fn vec4_map<F, T>(v: Vector4, func: F) -> [T; 4]
where
    F: Fn(f32) -> T,
{
    [func(v[0]), func(v[1]), func(v[2]), func(v[3])]
}

pub fn to_vec4(v: Vector3) -> Vector4 {
    [v[0], v[1], v[2], 0.]
}

pub fn to_vec3<T: Copy + num_traits::Zero>(v: &[T]) -> vecmath::Vector3<T> {
    let x = if !v.is_empty() { v[0] } else { T::zero() };
    let y = if v.len() > 1 { v[1] } else { T::zero() };
    let z = if v.len() > 2 { v[2] } else { T::zero() };
    [x, y, z]
}

pub fn dist(l: Vector3, r: Vector3) -> f32 {
    let d = vecmath::vec3_sub(l, r);
    vecmath::vec3_dot(d, d).sqrt()
}

pub fn mat4_scale(s: f32) -> Matrix4 {
    [
        [s, 0., 0., 0.],
        [0., s, 0., 0.],
        [0., 0., s, 0.],
        [0., 0., 0., 1.],
    ]
}

pub fn mat4_transform_vec3(m: Matrix4, t: Vector3) -> Vector3 {
    let r = vecmath::col_mat4_transform(m, to_vec4(t));
    to_vec3(&r)
}

pub fn mat4_t(pos: Vector3) -> Matrix4 {
    [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [pos[0], pos[1], pos[2], 1.],
    ]
}

pub fn mat4_ts(pos: Vector3, scale: f32) -> Matrix4 {
    [
        [scale, 0., 0., 0.],
        [0., scale, 0., 0.],
        [0., 0., scale, 0.],
        [pos[0], pos[1], pos[2], 1.],
    ]
}

pub fn mat4_rot(rot: Quaterion) -> Matrix4 {
    let x = rot.1[0];
    let y = rot.1[1];
    let z = rot.1[2];
    let w = rot.0;
    [
        [
            1. - 2. * y * y - 2. * z * z,
            2. * x * y + 2. * w * z,
            2. * x * z - 2. * w * y,
            0.,
        ],
        [
            2. * x * y - 2. * w * z,
            1. - 2. * x * x - 2. * z * z,
            2. * y * z + 2. * w * x,
            0.,
        ],
        [
            2. * x * z + 2. * w * y,
            2. * y * z - 2. * w * x,
            1. - 2. * x * x - 2. * y * y,
            0.,
        ],
        [0., 0., 0., 1.],
    ]
}

pub fn quaternion_to(vec: Vector3, to: Vector3) -> Quaterion {
    let a = vecmath::vec3_normalized(vec);
    let b = vecmath::vec3_normalized(to);

    let c = vecmath::vec3_cross(b, a);
    let c = vecmath::vec3_normalized(c);
    if !vec3_is_valid(c) {
        return (1.0, [0.0, 0.0, 0.0]);
    }

    let eps = 1e-4;
    let ip = vecmath::vec3_dot(a, b);
    if vecmath::vec3_len(c) < eps || 1.0 < ip {
        if ip < (eps - 1.0) {
            let a2 = [-a[1], a[2], a[0]];
            let c = vecmath::vec3_normalized(vecmath::vec3_cross(a2, a));
            (0.0, c)
        } else {
            (1.0, [0.0, 0.0, 0.0])
        }
    } else {
        let e = vecmath::vec3_scale(c, (0.5 * (1.0 - ip)).sqrt());
        ((0.5 * (1.0 + ip)).sqrt(), e)
    }
}

pub fn vec3_is_valid(v: Vector3) -> bool {
    !v[0].is_nan() && !v[1].is_nan() && !v[2].is_nan()
}
