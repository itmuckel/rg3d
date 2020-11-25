#version 330 core

layout(location = 0) in vec3 vertexPosition;

uniform mat4 worldViewProjection;

void main()
{
    gl_Position = worldViewProjection * vec4(vertexPosition, 1.0);
}
