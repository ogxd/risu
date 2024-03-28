using Microsoft.AspNetCore.Server.Kestrel.Core;
using Risu.Demo.GrpcService.Services;

var builder = WebApplication.CreateBuilder(args);

builder.WebHost.ConfigureKestrel(serverOptions =>
{
    serverOptions.ListenAnyIP(8501, listenOptions => listenOptions.Protocols = HttpProtocols.Http2);
});

// Add services to the container.
builder.Services.AddGrpc();

var app = builder.Build();

app.Use(async (context, next) =>
{
    Console.WriteLine("Received request at " + context.Request.Path + " from " + context.Connection.RemoteIpAddress);
    Console.WriteLine("  Content type: " + context.Request.ContentType);
    await next();
});

// Configure the HTTP request pipeline.
app.MapGrpcService<GreeterService>();
app.MapGet("/", () => "Communication with gRPC endpoints must be made through a gRPC client. To learn how to create a client, visit: https://go.microsoft.com/fwlink/?linkid=2086909");

app.Run();
