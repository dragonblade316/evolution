# SOTM trajectory math

The shot solver models the projectile in a vertical 2D plane aligned with the target. It accounts for gravity, the robot's velocity toward the target, and quadratic aerodynamic drag. Air is assumed to be stationary.

## Projectile constants

| Quantity | Symbol | Value |
|---|---:|---:|
| Air density | $\rho$ | $1.204\ \mathrm{kg/m^3}$ |
| Ball radius | $r$ | $0.15\ \mathrm{m}$ |
| Ball mass | $m$ | $0.283\ \mathrm{kg}$ |
| Gravity | $g$ | $9.81\ \mathrm{m/s^2}$ |
| Drag coefficient | $C_d$ | Configured by `drag_coefficient`, default `0.5` |

The ball's frontal area is:

$$
A = \pi r^2
$$

## Quadratic drag

The magnitude of aerodynamic drag is:

$$
F_d = \frac{1}{2}\rho C_d A v^2
$$

Drag points opposite the projectile velocity. Its vector form is therefore:

$$
\vec F_d = -\frac{1}{2}\rho C_d A |\vec v|\vec v
$$

Dividing by the ball mass gives drag acceleration:

$$
\vec a_d = -\frac{\rho C_d A}{2m}|\vec v|\vec v
$$

For velocity $\vec v = [v_x, v_y]$:

$$
|\vec v| = \sqrt{v_x^2 + v_y^2}
$$

$$
a_x = -k|\vec v|v_x
$$

$$
a_y = -g-k|\vec v|v_y
$$

where:

$$
k = \frac{\rho C_d A}{2m}
$$

At the default $C_d=0.5$, $k \approx 0.0752\ \mathrm{m^{-1}}$. At $10\ \mathrm{m/s}$, the drag acceleration magnitude is approximately $7.52\ \mathrm{m/s^2}$.

## First-order integration state

The physical trajectory is a second-order system because acceleration determines position. The `ivp::SecondOrderSystem` interface only permits acceleration to depend on time and position, while drag depends on velocity. The solver therefore expresses the same physics as a first-order state:

$$
\vec y = [x, y, v_x, v_y]
$$

$$
\frac{d\vec y}{dt} = [v_x, v_y, a_x, a_y]
$$

This lets the integrator use the current velocity when calculating drag.

## Initial velocity

The range solver guesses muzzle speed $s$ and time of flight $T$. For hood angle $\alpha$ and turret velocity $v_{\parallel}$ toward the target, the initial field-relative velocity is:

$$
v_x(0) = s\cos(\alpha) + v_{\parallel}
$$

$$
v_y(0) = s\sin(\alpha)
$$

The initial integration state is:

$$
\vec y(0) = [0, 0, v_x(0), v_y(0)]
$$

The trajectory is integrated to $T$. Gauss-Newton adjusts $s$ and $T$ to drive the final position errors toward zero:

$$
e_x = x(T) - x_{target}
$$

$$
e_y = y(T) - y_{target}
$$

The converged $s$ is the requested muzzle velocity.

## Lateral lead

The existing SOTM pipeline handles lateral robot velocity in a separate pass. After obtaining an initial time of flight, it estimates lateral displacement:

$$
L = v_{\perp}T
$$

The turret correction points opposite that displacement:

$$
\theta = \operatorname{atan2}(-L, d)
$$

where $d$ is the ground-plane distance to the target. The corrected horizontal range is:

$$
R = \sqrt{d^2 + L^2}
$$

The range solver then runs again with $R$ to calculate the final muzzle velocity.

## Model limitation

Drag is integrated in the vertical plane and affects range, height, and time of flight. The lateral correction remains the solver's existing kinematic approximation; lateral velocity is not included in the drag magnitude. A fully coupled model would integrate a 3D state and solve muzzle speed, turret correction, and flight time simultaneously.
