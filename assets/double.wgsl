
fn random(v: f32) -> f32 {
	return fract(sin(dot(vec2<f32>(v), vec2<f32>(12.9898,78.233))) * 43758.5453123);
}

fn random2d(v: vec2<f32>) -> f32 {
	return fract(sin(dot(v, vec2<f32>(12.9898,78.233))) * 43758.5453123);
}

// Sometimes needed for noise functions that sample multiple corners.
fn random2di(v: vec2<f32>) -> f32 {
	return random2d(floor(v));
}

fn cubic_hermite_curve(x: f32) -> f32 {
	//y = x*x*(3.0-2.0*x);
  return smoothstep(0., 1., x);
}

fn cubic_hermite_curve_2d(x: vec2<f32>) -> vec2<f32> {
	//y = x*x*(3.0-2.0*x);
  return smoothstep(vec2<f32>(0.), vec2<f32>(1.), x);
}
fn vnoise2d(v: vec2<f32>) -> f32 {
	let i = floor(v);
	let f = fract(v);

	// corners.
	let a = random2di(i);
	let b = random2di(i + vec2<f32>(1.0, 0.0));
	let c = random2di(i + vec2<f32>(0.0, 1.0));
	let d = random2di(i + vec2<f32>(1.0, 1.0));

	// Smooth
  let u = cubic_hermite_curve_2d(f);

	// Mix
	return mix(a, b, u.x) +
		(c - a) * u.y * (1.0 - u.x) +
		(d - b) * u.x * u.y;
}

let m2 = mat2x2<f32>(vec2<f32>(0.8, 0.6), vec2<f32>(-0.6, 0.8));
fn fbm(p: vec2<f32>) -> f32 {
	var p = p;
  var f = 0.;
  f = f + 0.5000 * vnoise2d(p); p = m2 * p * 2.02;
  f = f + 0.2500 * vnoise2d(p); p = m2 * p * 2.03;
  f = f + 0.1250 * vnoise2d(p); p = m2 * p * 2.01;
  f = f + 0.0625 * vnoise2d(p);
  return f / 0.9375 * 0.255;
}

struct ShaderInput {
    coord: vec2<f32>
}

@group(0) @binding(0)
var<uniform> input: ShaderInput;

struct Output {
    data: array<vec4<f32>>,
}

@group(0) @binding(1)
var<storage, read_write> output: Output;

@compute @workgroup_size(8,8,1)
fn double(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let CHUNK_SIZE = 128u;
    let location = vec2<i32>(i32(invocation_id.x), i32(invocation_id.y));
    let color = vec4(0.0, 1.0, 0.0, 1.0);
    let a = invocation_id.x + invocation_id.y * CHUNK_SIZE;
    let noise = fbm(vec2<f32>(f32(invocation_id.x), f32(invocation_id.y)));

    output.data[a] = vec4<f32>(
    f32(noise),
    f32(0.0),
    f32(0.0),
    1.0
    );
}