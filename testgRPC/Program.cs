using Grpc.Net.Client;
using Tk75Attractions.Struckout.V1;

var channel = GrpcChannel.ForAddress("");

var client = new MasterToProjectorService.MasterToProjectorServiceClient(channel);

var gameStartRequest = new StartGameRequest();
gameStartRequest.Difficulty = Difficulty.Normal;

var responce = await client.StartGameAsync(gameStartRequest);

Console.WriteLine("StartCorrectly");