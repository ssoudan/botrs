# Botrs 

Fun with chatGPT API, [llm-chain](https://github.com/sobelio/llm-chain) and 
[huelib (fork)](https://github.com/ssoudan/huelib-rs/).

## Disclaimer

This will probably try to take over the world and if it doesn't succeed, 
empty your bank account and kill all the remaining polar bears trying to do so.

## Usage

`.env` file with: 
```
OPENAI_API_KEY=...
HUE_USERNAME=...
HUE_BRIDGE_IP=...
```
Look at `src/main.rs` if you don't already have these.

# Example of 'successful' run

```
$ cargo run
```
Compiling botrs v0.1.0 (ssoudan/botrs)

### Warm up

[... half a decade of compilation later ...]

System:
<pre><code>
You are an automated agent named botGPT interacting with the WORLD
</code></pre>

WORLD:
<pre><code>
You are botGPT, large language model assisting the WORLD. Use available tools to answer the question as best as you can.
You will proceed in a OODA loop made of the following steps:
- Observations: What do you know? What is your source? What don't you know? You might want to note down important information for later like relevant past Action results. 
- Orientation: Plan the intermediate objectives along the path to answer the original question. Make a list of current objectives. 
- Decision: Choose what to do first to answer the question. Why? What are the pros and cons of this decision?
- Action: Take a single Action consisting of exactly one tool invocation. The available Tools listed below. Use ConcludeTool when you have the final answer to the original question.

# Notes: 
- Template expansion is not supported in Action. If you need to pass information from on action to another, you have to pass them manually.
- Use ConcludeTool with your conclusion (as text) once you have the final answer to the original question.
- There are no APIs available to you. You have to use the Tools.

# Format of your response

Please use the following format for your response - no need to be verbose:
====================
## Observations:
- ...
## Orientation:
- ...
## Decision:
- ...
## The ONLY Action: <Do not give multiple command. Only one per response.>
```yaml
command: <ToolName>
input:
  <... using the `input_format` for the Tool ...>
```
====================

# The following are the ONLY Tools you can use for your Actions:
- name: RoomTool
  description: A tool to use that the source of truth for the Lights of a Room.
  description_context: Use this to fetch the Lights of Rooms.
  input_format:
    room_filter: 'The list of Room names (<string>) to get the lights for, e.g. `room_filter: ["Bedroom"]`. If unsure, use `[]` to get all Rooms.'
- name: ConcludeTool
  description: A tool to terminate a task with a conclusion.
  description_context: Use this to terminate a task when it is complete.
  input_format:
    conclusion: The final textual answer for this task. No string interpolation supported, only plain text. MANDATORY.
- name: SandboxedPythonTool
  description: 'A tool that executes sandboxed Python code. Only stdout and stderr are captured and made available. '
  description_context: Use this to transform data. This is not a tool to retrieve information. Except `print()`, no interactions with the world. No input. No `import`. No library. No API access. Just plain Python. import|open|exec|eval|__import__ are forbidden.
  input_format:
    code: 'The Python code to execute. For example: `data = [...]; <...>; output = <...> ; print(output)`. MANDATORY'
- name: LightStatusTool
  description: A tool to use that the source of truth for the Light statuses.
  description_context: Use this to fetch the Light statuses
  input_format:
    light_filter: 'The list of Lights IDs (<string>) to get the status for, e.g. `light_filter: ["1", "2"]`. Use `[]` to get all Lights.'

</code></pre>

Bot:
<pre><code>
Understood.
</code></pre>

### Now the real thing

WORLD:
<pre><code>
## Original question: Sort in ascending order: [2, 3, 1, 4, 5].
</code></pre>

Bot:
<pre><code>
## Observations:
- The given list to sort is [2, 3, 1, 4, 5].
- I need to sort this list in ascending order.
## Orientation:
- SandboxedPythonTool can be used to sort the list.
## Decision:
- We can use the sorted() function of Python to sort the list.
## The ONLY Action:
```yaml
command: SandboxedPythonTool
input:
  code: |
    lst = [2, 3, 1, 4, 5]
    sorted_list = sorted(lst)
    print(f"The sorted list is {sorted_list}")
```
</code></pre>

WORLD:
<pre><code>
# Action result:
```yaml
status: 0
stdout: |
  The sorted list is [1, 2, 3, 4, 5]
stderr: ''
```
# Your turn
Original question: Sort [2, 3, 1, 4, 5]
Observations, Orientation, Decision, The ONLY Action?
</code></pre>

Bot:
<pre><code>
# Observations:
- We needed to sort the list in ascending order.
- We have the result of the Action.
- We have the sorted list: [1, 2, 3, 4, 5].
# Orientation:
- I know the answer to the original question.
# Decision:
- Use the ConcludeTool to terminate the task with the sorted list.
# The ONLY Action:
```yaml
command: ConcludeTool
input:
  conclusion: |
    The ascending sorted list is [1, 2, 3, 4, 5].
```
</code></pre>

WORLD:
<pre><code>
# Your turn
Original question: What is the status of the lights where I most likely work?
Observations, Orientation, Decision, The ONLY Action?
</code></pre>

Bot:
<pre><code>
# Observations:
- We need to find the status of the lights where the user most likely works.
- We need information about the room where the user works to query the LightStatusTool.
# Orientation:
- We can use the RoomTool to find the name of the room where the user works.
- Then use this room name as a filter with the LightStatusTool to get the status of the lights.
# Decision:
- Use the RoomTool to find the name of the room where the user works.
- Use the LightStatusTool to find the status of the lights in the room.
# The ONLY Action:
```yaml
command: RoomTool
input:
  room_filter: ["office"]
```
</code></pre>

WORLD:
<pre><code> 
# Action result: 
```yaml
rooms: []
```
# Your turn
Original question: What is the status of the lights where I most likely work?
Observations, Orientation, Decision, The ONLY Action?
</code></pre>

Bot:
<pre><code> 
# Observations:
- We need to find the status of the lights where the user most likely works.
- The RoomTool with the room_filter of "office" returned an empty list of rooms.
- We don't know which room we should look at now.
# Orientation:
- We can use the RoomTool to fetch the list of rooms and their details.
- We can then decide which room to consider.
# Decision:
- Use the RoomTool to fetch the list of all rooms and their details.
# The ONLY Action:
```yaml
command: RoomTool
input:
  room_filter: []
```
</code></pre>

WORLD:
<pre><code> 
# Action result: 
```yaml
rooms:
- name: Living
  lights:
  - '4'
  - '5'
  - '6'
  - '21'
- name: Office
  lights:
  - '13'
  - '27'
  - '2'
  - '16'
  - '26'
  - '25'
  - '24'
  - '15'
  - '17'
  - '18'
  - '20'
- name: Hallway
  lights:
  - '33'
  - '28'
- name: Room
  lights:
  - '32'
  - '3'
  - '1'
  - '9'
  - '10'
  - '11'
  - '19'
- name: Kitchen
  lights:
  - '31'
  - '30'
  - '22'
  - '23'
```
# Your turn
Original question: What is the status of the lights where I most likely work?
Observations, Orientation, Decision, The ONLY Action?
</code></pre>

Bot:
<pre><code> 
# Observations:
- We need to find the status of the lights where the user most likely works.
- We got a list of rooms and their light IDs from the RoomTool.
- We need to identify which room the user most likely works in before we can look up the status of the lights.
# Orientation:
- To identify the room where the user works, we need more information about the user.
- If room name is not available, we can use the LightStatusTool to query for the status of lights in all rooms.
# Decision:
- Use the LightStatusTool to get the status of lights in all rooms.
# The ONLY Action:
```yaml
command: LightStatusTool
input:
  light_filter: []
```
</code></pre>

WORLD:
<pre><code> 
# Action result: 
```yaml
lights:
- id: '23'
  name: Bloom L
  state:
    on: false
    brightness: 254
    hue: 0
    saturation: 143
- id: '27'
  name: Desk lamp
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 200
    color_temperature: 500
- id: '28'
  name: Couloir south
  state:
    on: false
    brightness: 56
    hue: 21845
    saturation: 199
    color_temperature: 153
- id: '31'
  name: Kitchen
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 199
    color_temperature: 153
- id: '19'
  name: Sapin 4
  state:
    on: false
    brightness: 254
    hue: 0
    saturation: 199
    color_temperature: 500
- id: '22'
  name: Bloom R
  state:
    on: false
    brightness: 254
    hue: 65535
    saturation: 143
- id: '26'
  name: Work R
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 199
    color_temperature: 153
- id: '16'
  name: Under desk lightstrip
  state:
    on: false
    brightness: 254
    hue: 0
    saturation: 199
    color_temperature: 500
- id: '1'
  name: Iris
  state:
    on: false
    brightness: 254
    hue: 0
    saturation: 143
- id: '10'
  name: Sapin 3
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 200
    color_temperature: 500
- id: '13'
  name: Walk-in closet
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 200
    color_temperature: 328
- id: '18'
  name: Bureau C
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 199
    color_temperature: 500
- id: '2'
  name: Spot bureau
  state:
    on: false
    brightness: 254
    hue: 65535
    saturation: 200
    color_temperature: 500
- id: '25'
  name: Work L
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 199
    color_temperature: 500
- id: '5'
  name: Salon 3
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 200
    color_temperature: 328
- id: '24'
  name: Bureau D
  state:
    on: false
    brightness: 254
    hue: 65535
    saturation: 199
    color_temperature: 500
- id: '15'
  name: Desk bloom
  state:
    on: false
    brightness: 254
    hue: 0
    saturation: 143
- id: '6'
  name: Salon 2
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 200
    color_temperature: 500
- id: '21'
  name: Counter
  state:
    on: false
    brightness: 254
    hue: 65535
    saturation: 199
    color_temperature: 500
- id: '17'
  name: Bureau A
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 199
    color_temperature: 153
- id: '30'
  name: Sink
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 199
    color_temperature: 500
- id: '9'
  name: Sapin 2
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 200
    color_temperature: 328
- id: '20'
  name: Bureau B
  state:
    on: false
    brightness: 254
    hue: 65535
    saturation: 199
    color_temperature: 500
- id: '3'
  name: Bed lightstrip
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 143
- id: '11'
  name: Table
  state:
    on: false
    brightness: 254
    hue: 65535
    saturation: 200
    color_temperature: 500
- id: '32'
  name: Sapin 1
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 254
    color_temperature: 447
- id: '33'
  name: Couloir Nord
  state:
    on: false
    brightness: 56
    hue: 0
    saturation: 199
    color_temperature: 500
- id: '4'
  name: Salon 1
  state:
    on: false
    brightness: 254
    hue: 0
    saturation: 200
    color_temperature: 500
```
# Your turn
Original question: What is the status of the lights where I most likely work?
Observations, Orientation, Decision, The ONLY Action?
</code></pre>

Bot:
<pre><code> 
# Observations:
- We need to find the status of the lights where the user most likely works.
- We got the status of all lights from LightStatusTool.
- We need to identify which room the user most likely works in before we can look up the status of the lights.
# Orientation:
- We can use the RoomTool to fetch the list of all rooms and check which room has a light that the user uses most often.
- We can then use the LightStatusTool to query the status of lights in that room.
# Decision:
- Use the RoomTool to fetch the list of all rooms.
- Check which room has a light that the user uses most often.
- Use the LightStatusTool to query the status of lights in that room.
# The ONLY Action:
```yaml
command: RoomTool
input:
  room_filter: []
```
</code></pre>

WORLD:
<pre><code> 
# Action result: 
```yaml
rooms:
- name: Kitchen
  lights:
  - '31'
  - '30'
  - '22'
  - '23'
- name: Office
  lights:
  - '13'
  - '27'
  - '2'
  - '16'
  - '26'
  - '25'
  - '24'
  - '15'
  - '17'
  - '18'
  - '20'
- name: Room
  lights:
  - '32'
  - '3'
  - '1'
  - '9'
  - '10'
  - '11'
  - '19'
- name: Living
  lights:
  - '4'
  - '5'
  - '6'
  - '21'
- name: Hallway
  lights:
  - '33'
  - '28'
```
# Your turn
Original question: What is the status of the lights where I most likely work?
Observations, Orientation, Decision, The ONLY Action?
</code></pre>

Bot:
<pre><code> 
# Observations:
- We need to find the status of the lights where the user most likely works.
- We got the list of rooms where the user most likely works from the RoomTool.
# Orientation:
- We can see that the Office room in the fetched list contains lights, so we can use the LightStatusTool to look for the status of the lights in the Office room.
# Decision:
- Use the LightStatusTool to find the status of the lights in the Office room.
# The ONLY Action:
```yaml
command: LightStatusTool
input:
  light_filter: ["13", "27", "2", "16", "26", "25", "24", "15", "17", "18", "20"]
```
</code></pre>

WORLD:
<pre><code> 
# Action result: 
```yaml
lights:
- id: '26'
  name: Work R
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 199
    color_temperature: 153
- id: '16'
  name: Under desk lightstrip
  state:
    on: false
    brightness: 254
    hue: 0
    saturation: 199
    color_temperature: 500
- id: '25'
  name: Work L
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 199
    color_temperature: 500
- id: '27'
  name: Desk lamp
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 200
    color_temperature: 500
- id: '2'
  name: Spot bureau
  state:
    on: false
    brightness: 254
    hue: 65535
    saturation: 200
    color_temperature: 500
- id: '24'
  name: Bureau D
  state:
    on: false
    brightness: 254
    hue: 65535
    saturation: 199
    color_temperature: 500
- id: '15'
  name: Desk bloom
  state:
    on: false
    brightness: 254
    hue: 0
    saturation: 143
- id: '20'
  name: Bureau B
  state:
    on: false
    brightness: 254
    hue: 65535
    saturation: 199
    color_temperature: 500
- id: '17'
  name: Bureau A
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 199
    color_temperature: 153
- id: '18'
  name: Bureau C
  state:
    on: false
    brightness: 254
    hue: 43690
    saturation: 199
    color_temperature: 500
- id: '13'
  name: Walk-in closet
  state:
    on: false
    brightness: 254
    hue: 21845
    saturation: 200
    color_temperature: 328
```
# Your turn
Original question: What is the status of the lights where I most likely work?
Observations, Orientation, Decision, The ONLY Action?
</code></pre>

Bot:
<pre><code> 
# Observations:
- We used the LightStatusTool to fetch the status of lights in the Office room.
# Orientation:
- We can see from the retrieved data that all lights in the Office room are off, so we can inform the user that no lights are on in the Office room.
# Decision:
- Inform the user.
# The ONLY Action:
```yaml
command: ConcludeTool
input:
  conclusion: |
    All the lights in the user's most likely work place are currently off.
```
</code></pre>

Tadam:

<pre><code> 
And the conclusion is: All the lights in the user's most likely work place are currently off. 

</code></pre>

Not bad.