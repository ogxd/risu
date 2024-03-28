using System.Diagnostics;
using System.Net;
using Microsoft.AspNetCore.Server.Kestrel.Core;
using Yarp.ReverseProxy.Forwarder;

var builder = WebApplication.CreateBuilder(args);

builder.WebHost.ConfigureKestrel(serverOptions =>
{
    //https://learn.microsoft.com/en-us/aspnet/core/grpc/aspnetcore?view=aspnetcore-8.0&tabs=visual-studio#protocol-negotiation
    serverOptions.ListenAnyIP(8500, listenOptions => listenOptions.Protocols = HttpProtocols.Http2);
});

builder.Services.AddHttpForwarder();

var app = builder.Build();

//app.UseHttpsRedirection();

app.Use(async (context, next) =>
{
    Console.WriteLine("Received request at " + context.Request.Path + " from " + context.Connection.RemoteIpAddress);
    Console.WriteLine("  Content type: " + context.Request.ContentType);
    await next();
});

app.MapGet("/", async context =>
{
    await context.Response.WriteAsync("Hello World!");
});

// Configure our own HttpMessageInvoker for outbound calls for proxy operations
var httpClient = new HttpMessageInvoker(new SocketsHttpHandler()
{
    UseProxy = false,
    AllowAutoRedirect = false,
    AutomaticDecompression = DecompressionMethods.None,
    UseCookies = false,
    ActivityHeadersPropagator = new ReverseProxyPropagator(DistributedContextPropagator.Current),
    ConnectTimeout = TimeSpan.FromSeconds(15),
});

// Setup our own request transform class
var transformer = HttpTransformer.Default;
var requestConfig = new ForwarderRequestConfig
{
    ActivityTimeout = TimeSpan.FromSeconds(100),
    Version = HttpVersion.Version20,
    VersionPolicy = HttpVersionPolicy.RequestVersionExact,
};

app.Map("/{**catch-all}", async (HttpContext context, IHttpForwarder forwarder) =>
{
    var error = await forwarder.SendAsync(context, "http://localhost:8501/",
        httpClient, requestConfig, transformer);

    if (error != ForwarderError.None)
    {
        var errorFeature = context.GetForwarderErrorFeature();
        var exception = errorFeature.Exception;
    }
});

app.Run();
