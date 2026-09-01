using Google.Protobuf;
using Tk75Attractions.Struckout.V1;
using Struckout.Application;
using Struckout.Infrastructure;

Console.WriteLine("Hello, World!");
IServerService<ProjectorPacket> clientProjectorService = new TCPClientBase<ProjectorPacket>();
IServerService<MasterPacket> masterProjectorService = new TCPClientBase<MasterPacket>();
bool isOpen = true;

while (isOpen)
{
    var line = Console.ReadLine();

    if (string.IsNullOrWhiteSpace(line))
        continue;

    var arguments = line.Split(" ");

    var command = arguments[0];

    switch (command)
    {
        case "create":
            {
                var numString = arguments[1];
                var type = arguments[2];
                if (string.IsNullOrWhiteSpace(line)) Console.WriteLine("Num arguments can't be null");
                if (!int.TryParse(numString, out int num))
                {
                    Console.WriteLine("ERROR: Can't parse to int");
                }

                switch (type)
                {
                    case "sensor":
                        {
                            clientProjectorService.RegisterPort(num);
                            Console.WriteLine("Successfully Register");
                            break;
                        }
                    case "master":
                        {
                            masterProjectorService.RegisterPort(num);
                            Console.WriteLine("Successfully Register");
                            break;
                        }
                    default:
                        {
                            Console.WriteLine("There is no type");
                            break;
                        }
                }
                break;
            }
        case "connect":
            {
                var type = arguments[1];
                if (string.IsNullOrWhiteSpace(line)) Console.WriteLine("Num arguments can't be null");

                switch (type)
                {
                    case "sensor":
                        {
                            await clientProjectorService.Open();
                            Console.WriteLine("Successfully Open");
                            break;
                        }
                    case "master":
                        {
                            await masterProjectorService.Open();
                            Console.WriteLine("Successfully Open");
                            break;
                        }
                    default:
                        {
                            Console.WriteLine("There is no type");
                            break;
                        }
                }
                break;
            }
        case "close":
            {
                {
                    var type = arguments[1];
                    if (string.IsNullOrWhiteSpace(line)) Console.WriteLine("Num arguments can't be null");

                    switch (type)
                    {
                        case "sensor":
                            {
                                await clientProjectorService.Close();
                                Console.WriteLine("Successfully Close");
                                break;
                            }
                        case "master":
                            {
                                await masterProjectorService.Close();
                                Console.WriteLine("Succesfully Close");
                                break;
                            }
                        default:
                            {
                                Console.WriteLine("There is no type");
                                break;
                            }
                    }
                    break;
                }
            }
        case "checkopen":
            {
                if (clientProjectorService.GetOpen()) Console.WriteLine($"client: {clientProjectorService.GetPort()}");
                if (masterProjectorService.GetOpen()) Console.WriteLine($"master: {masterProjectorService.GetPort()}");
                Console.WriteLine("Check output");
                break;
            }
        case "exit":
            {
                isOpen = false;
                break;
            }
        default:
            {
                Console.WriteLine("Undefined control");
                break;
            }
    }
}

if (clientProjectorService.GetOpen())
{
    await clientProjectorService.Close();
    Console.WriteLine("Succesfully Close");
}
if (masterProjectorService.GetOpen())
{ 
    await masterProjectorService.Close();
    Console.WriteLine("Succesfully Close");
}
