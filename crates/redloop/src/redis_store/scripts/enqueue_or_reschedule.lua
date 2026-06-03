local failures = KEYS[1]
local ready = KEYS[2]
local scheduled = KEYS[3]
local leased = KEYS[4]
local failed = KEYS[5]
local rerun = KEYS[6]

local op = ARGV[1]
local job_id = ARGV[2]
local now_us = tonumber(ARGV[3])
local requested_us = tonumber(ARGV[4]) or 0
local replace_mode = ARGV[5]

local function current_state()
    local ready_score = redis.call('ZSCORE', ready, job_id)
    if ready_score then
        return 'ready', tonumber(ready_score)
    end

    local scheduled_score = redis.call('ZSCORE', scheduled, job_id)
    if scheduled_score then
        return 'scheduled', tonumber(scheduled_score)
    end

    local leased_score = redis.call('ZSCORE', leased, job_id)
    if leased_score then
        return 'leased', tonumber(leased_score)
    end

    local failed_score = redis.call('ZSCORE', failed, job_id)
    if failed_score then
        return 'failed', tonumber(failed_score)
    end

    return 'none', 0
end

local function normalize_target()
    if requested_us == 0 or requested_us <= now_us then
        return 'ready', now_us, 0
    end

    return 'scheduled', requested_us, requested_us
end

local function int_string(value)
    return string.format('%.0f', value)
end

local function remove_waiting(state)
    if state == 'ready' then
        redis.call('ZREM', ready, job_id)
    elseif state == 'scheduled' then
        redis.call('ZREM', scheduled, job_id)
    end
end

local function add_target(state, score)
    if state == 'ready' then
        redis.call('ZADD', ready, score, job_id)
    else
        redis.call('ZADD', scheduled, score, job_id)
    end
end

local function should_replace(target_score, existing_score)
    if replace_mode == 'earlier' then
        return target_score < existing_score
    end
    if replace_mode == 'later' then
        return target_score > existing_score
    end
    return true
end

local state, score = current_state()
local was_failed = state == 'failed'

if op == 'reschedule' then
    if state == 'none' then
        return {'err', 'not_found'}
    end
    if state == 'leased' or state == 'failed' then
        return {'err', 'invalid_state', state}
    end

    local target_state, target_score, schedule_value = normalize_target()
    if not should_replace(target_score, score) then
        local existing_schedule = 0
        if state == 'scheduled' then
            existing_schedule = score
        end
        return {'ok', 'unchanged', state, int_string(existing_schedule)}
    end

    if state == target_state and score == target_score then
        return {'ok', 'unchanged', state, int_string(schedule_value)}
    end

    remove_waiting(state)
    redis.call('SREM', rerun, job_id)
    add_target(target_state, target_score)
    return {'ok', 'updated', target_state, int_string(schedule_value)}
end

if was_failed then
    redis.call('ZREM', failed, job_id)
    redis.call('HDEL', failures, job_id)
    redis.call('SREM', rerun, job_id)
    state = 'none'
    score = 0
end

if op == 'enqueue_immediate' then
    if state == 'ready' then
        return {'ok', 'unchanged', state, '0'}
    end

    if state == 'leased' then
        local added = redis.call('SADD', rerun, job_id)
        if added == 1 then
            return {'ok', 'updated', state, '0'}
        end
        return {'ok', 'unchanged', state, '0'}
    end

    if state == 'scheduled' then
        redis.call('ZREM', scheduled, job_id)
        redis.call('SREM', rerun, job_id)
        redis.call('ZADD', ready, now_us, job_id)
        return {'ok', 'updated', 'ready', '0'}
    end

    redis.call('ZADD', ready, now_us, job_id)
    if was_failed then
        return {'ok', 'updated', 'ready', '0'}
    end
    return {'ok', 'created', 'ready', '0'}
end

if state == 'leased' then
    return {'err', 'invalid_state', 'leased'}
end

local target_state, target_score, schedule_value = normalize_target()
if state == 'none' then
    redis.call('SREM', rerun, job_id)
    add_target(target_state, target_score)
    if was_failed then
        return {'ok', 'updated', target_state, tostring(schedule_value)}
    end
    return {'ok', 'created', target_state, int_string(schedule_value)}
end

if state == target_state and score == target_score then
    return {'ok', 'unchanged', state, int_string(schedule_value)}
end

if not should_replace(target_score, score) then
    local existing_schedule = 0
    if state == 'scheduled' then
        existing_schedule = score
    end
    return {'ok', 'unchanged', state, int_string(existing_schedule)}
end

remove_waiting(state)
redis.call('SREM', rerun, job_id)
add_target(target_state, target_score)
return {'ok', 'updated', target_state, int_string(schedule_value)}
