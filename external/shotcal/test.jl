using JuMP, MadNLP
# using AMDGPU
using LinearAlgebra
# using Pkg
# Pkg.add("Interpolations")
#
struct Config
    field_length::Float64
    field_width::Float64
    shooter_reletive::Array{Float64,1}
    timesteps::Int
    target_pos::Array{Float64,1}

    ball_mass::Float64
    ball_diameter::Float64
    air_density::Float64
    drag_coef::Float64
    gravity::Array{Float64,1}
end

function solve(robot_pos_vel, config::Config)
    shooter_pos_vel = robot_pos_vel + config.shooter_reletive

    #timesteps for each trajectory
    N = config.timesteps

    g = config.gravity

    model = Model()

    #tof var
    @variable(model, tof >= 0)
    dt = tof / N

    #If this ever becomes non linear than it may need adjustments to work with jump
    function dynamics(x)
        v = x[4:6]
        return vcat(v, -g)
    end

    #state matrix.
    @variable(model, x[1:6, 1:N])
    @constraint(model, x[1:3, 1] .== shooter_pos_vel[1:3])
    # if !ismissing(x_i)
    #     set_start_value(x, x_i)
    # end


    println("Generating guesses")
    for i in 1:N
        ratio = i / N

        pos_guess = shooter_pos_vel[1:3] + (config.target_pos - shooter_pos_vel[1:3]) * ratio
        set_start_value.(x[1:3, N], pos_guess)
        # println("running test")

        # vel_guess = initial[4:6] +

    end

    for i in 1:(N-1)
        X_k = @view x[:, i]
        X_k1 = @view x[:, i+1]

        k1 = dynamics(X_k)
        k2 = dynamics(X_k + (dt / 2) * k1)
        k3 = dynamics(X_k + (dt / 2) * k2)
        k4 = dynamics(X_k + (dt * k3))

        #RK4
        @constraint(model, X_k1 .== X_k + (dt / 6) * (k1 + 2k2 + 2k3 + k4))
    end

    #To ensure the ball is falling downward on the goal
    @constraint(model, x[6, N] <= 0)
    #The ball needs to land in the goal
    @constraint(model, x[1:3, N] .== config.target_pos)

    JuMP.set_optimizer(model, MadNLP.Optimizer)
    # JuMP.set_optimizer_attribute(model, "array_type", ROCArray)
    # JuMP.set_optimizer_attribute(model, "max_iter", 100)
    # JuMP.set_optimizer_attribute(model, "print_level", MadNLP.INFO)

    #rewrite
    #This part was made with ai bc I need to wrap this up.
    v0_shooter_rel = x[4:6, 1] - shooter_pos_vel[4:6]
    @objective(model, Min, dot(v0_shooter_rel, v0_shooter_rel))


    println("Starting up optimizor")
    optimize!(model)
    println("Optimization finished")

    #prints (just copied from the ai example)
    v0_val = value.(x[4:6, 1]) - shooter_pos_vel[4:6]
    speed = norm(v0_val)
    pitch = atan(v0_val[3], hypot(v0_val[1], v0_val[2]))
    yaw = atan(v0_val[2], v0_val[1])

    println("Solution Stats:")
    println("Velocity: ", round(speed, digits=3), " m/s")
    println("Pitch: ", round(rad2deg(pitch), digits=3), "°")
    println("Yaw: ", round(rad2deg(yaw), digits=3), "°")
    println("Total Time: ", round(value(tof), digits=3), " s")
    println("V_z initial: ", v0_val[3])
    println("Final pos: ", value.(x[1:3, N]))

    t = value(tof)
    v_0 = v0_val[3]

    println("New data: ", v_0 * t + 0.5 * (-9.81)(t^2))
    return value.(x)
end

# robot = [1.0, 1.0, 0.0, 0.0, 0.0, 0.0]
# @time solve(robot)
function main()
    config = Config(2.0, 2.0, [0, 0, 0.5, 0, 0, 0], 20, [0, 0, 0.05], 0, 0, 0, 0, [0, 0, 9.806])

    Threads.@threads for i in 0:0.01:config.field_length
        for j in 0:0.01:config.field_width
            println("cordinates: ", i, j)
            robot_pos_vel = [i, j, 0.0, 0.0, 0.0, 0.0]
            solve(robot_pos_vel, config)
        end
    end
end

@time main()
#4 threads two tloops 400secs
#32 threads two tloops no time given
