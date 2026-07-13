import math

import numpy as np

ball_drag_coeff = 0.5

field_width = 8.2296  # 27 ft -> m
field_length = 16.4592  # 54 ft -> m
target_wrt_field = np.array(
    [[field_length / 2.0], [field_width / 2.0], [2.64], [0.0], [0.0], [0.0]]
)
target_radius = 0.61  # m
cone_angle = math.pi / 4  # rad
g = np.array([[0], [0], [9.806]])  # m/s²
