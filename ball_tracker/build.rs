fn get_port_def(module_name: &str, server_name: &str) -> u16 {
    struckout_utils::yaml::get_port_def("../api/spec/servers.yaml", module_name, server_name)
        .expect("failed to get port definition")
}

fn main() {
    let video_port = get_port_def("ball-tracker", "camera-video");
    let data_port = get_port_def("ball-tracker", "camera-data");
    let ctrl_port = get_port_def("ball-tracker", "camera-ctrl");
    let projector_port = get_port_def("ball-tracker", "projector");
    println!("cargo::rustc-env=CAMERA_VIDEO_PORT={}", video_port);
    println!("cargo::rustc-env=CAMERA_DATA_PORT={}", data_port);
    println!("cargo::rustc-env=CAMERA_CTRL_PORT={}", ctrl_port);
    println!("cargo::rustc-env=PROJECTOR_PORT={}", projector_port);

    struckout_utils::buildrs::set_commit_info();
}
