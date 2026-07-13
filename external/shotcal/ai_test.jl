# import Pkg; Pkg.add("Plots")

using JuMP, MadNLP, AMDGPU
using LinearAlgebra

AMDGPU.allowscalar(true)
# using Plots

function main()
    # --- Constants ---
    field_width = 8.2296
    field_length = 16.4592
    target_radius = 0.61
    cone_angle = π / 4
    g_vec = [0.0, 0.0, 9.806]
    ρ = 1.204
    r_ball = 0.15
    A = π * r_ball^2
    m = 0.283
    C_D = 0.5
    C_L = 0.5
    ω = [0.0, -2.0, 0.0] # rad/s
    
    target_pos = [field_length / 2.0, field_width / 2.0, 2.64]
    robot_pos_vel = [field_length / 4.0, field_width / 4.0, 0.0, 1.524, -1.524, 0.0]
    shooter_wrt_robot = [0.0, 0.0, 1.2, 0.0, 0.0, 0.0]
    shooter_pos_vel = robot_pos_vel + shooter_wrt_robot

    # g_vec = ROCArray([0.0, 0.0, 9.806])
    # ω = ROCArray([0.0, -2.0, 0.0])
    # shooter_pos_vel = ROCArray(robot_pos_vel + shooter_wrt_robot)
    #
    max_v0 = 10.0
    N = 50 # Decision steps

    # --- Model Setup ---
    # Use MadNLP with AMDGPU (ROCArray)
    model = Model(MadNLP.Optimizer)
    set_optimizer_attribute(model, "array_type", Array)
    set_optimizer_attribute(model, "linear_solver", "lapack") # Dense solver for GPU
    #
    @variable(model, T >= 0, start = 1.0)
    @variable(model, X[1:6, 1:N])
    
    dt = T / N

    # --- Dynamics Function (F) ---
    function dynamics(x_vec)
      v = x_vec[4:6]
      # Use sqrt(dot(...)) or norm(...)
      v_mag = sqrt(dot(v, v) + 1e-6) 
      
      # Use ./ to divide each element of the vector by the scalar v_mag
      v_hat = v ./ v_mag  # <--- Change / to ./
      
      f_drag = 0.5 * ρ * (v_mag^2) * C_D * A
      f_lift = 0.5 * ρ * v_mag * C_L * A
      
      magnus_dir = cross(v, ω)
      
      # Use ./ here as well for the force-to-mass conversion
      accel = -g_vec .- (f_drag / m) .* v_hat .- (f_lift / m) .* magnus_dir
      
      return vcat(v, accel)

     end

    # --- Initial Guesses ---
    for k in 1:N
        frac = (k-1)/(N-1)
        # Linear interp for position
        for i in 1:3
            set_start_value(X[i, k], shooter_pos_vel[i] + frac * (target_pos[i] - shooter_pos_vel[i]))
        end
        # Velocity guess: robot vel + boost toward target
        dir = (target_pos - shooter_pos_vel[1:3]) / norm(target_pos - shooter_pos_vel[1:3])
        for i in 1:3
            set_start_value(X[i+3, k], robot_pos_vel[i+3] + max_v0 * dir[i])
        end
    end

    # --- Constraints ---
    
    # 1. Initial State (Shooter Position)
    @constraint(model, X[1:3, 1] .== shooter_pos_vel[1:3])

    # 2. Max Initial Velocity (relative to robot)
    v0_rel = X[4:6, 1] - robot_pos_vel[4:6]
    @constraint(model, dot(v0_rel, v0_rel) <= max_v0^2)

    # 3. Keep-out region (Cylinder with Conic Bowl)
    x_c, y_c = target_pos[1], target_pos[2]
    z_c = target_pos[3] - target_radius / tan(cone_angle)
    
    for k in 1:N
      # No 'model' or 'name' argument inside the macro
      x_dist_sq = @expression(model, (X[1, k] - x_c)^2 + (X[2, k] - y_c)^2)
      cyl = @expression(model, x_dist_sq - target_radius^2)
      cone = @expression(model, (X[3, k] - z_c)^2 * tan(cone_angle)^2 - x_dist_sq)
      
      # The solver will still see the full math here
      @constraint(model, cyl + cone + sqrt((cyl - cone)^2 + 1e-4) >= 0)
    end

    # 4. Dynamics (RK4)
    for k in 1:(N-1)
        # Using a loop for the 6 states to define RK4
        x_k = X[:, k]
        h = dt
        
        # Note: For complex NL dynamics in JuMP, we often define the steps explicitly
        # or use a User Defined Function. For simplicity in this block:
        k1 = dynamics(x_k)
        k2 = dynamics(x_k + (h/2) * k1)
        k3 = dynamics(x_k + (h/2) * k2)
        k4 = dynamics(x_k + h * k3)
        
        @constraint(model, X[:, k+1] .== x_k + (h/6) * (k1 + 2k2 + 2k3 + k4))
    end

    # 5. Final Conditions
    @constraint(model, X[1:3, N] .== target_pos)
    @constraint(model, X[6, N] <= 0) # Falling down

    # --- Objective ---
    # Minimize initial velocity squared (relative to shooter)
    v0_shooter_rel = X[4:6, 1] - shooter_pos_vel[4:6]
    @objective(model, Min, dot(v0_shooter_rel, v0_shooter_rel))

    # --- Solve ---
    optimize!(model)

    # --- Results ---
    v0_val = value.(X[4:6, 1]) - shooter_pos_vel[4:6]
    speed = norm(v0_val)
    pitch = atan(v0_val[3], hypot(v0_val[1], v0_val[2]))
    yaw = atan(v0_val[2], v0_val[1])

    println("Solution Stats:")
    println("Velocity: ", round(speed, digits=3), " m/s")
    println("Pitch: ", round(rad2deg(pitch), digits=3), "°")
    println("Yaw: ", round(rad2deg(yaw), digits=3), "°")
    println("Total Time: ", round(value(T), digits=3), " s")

    # --- Plotting ---
    traj_x = value.(X[1, :])
    traj_y = value.(X[2, :])
    traj_z = value.(X[3, :])
    # plot(traj_x, traj_y, traj_z, label="Trajectory", title="FRC 2022 Shooter Path", xlabel="X", ylabel="Y", zlabel="Z")
end

main()
