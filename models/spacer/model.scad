spacer(
    outer  = 30.0,
    inner  = 15.0,
    height = 10.0
);

module spacer(outer, inner, height) {
    difference() {
        cylinder(d = outer, h = height, $fn = 360);
        cylinder(d = inner, h = height, $fn = 360);
    }
}
