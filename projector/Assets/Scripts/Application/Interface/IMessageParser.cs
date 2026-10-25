namespace Struckout.Application.Interface
{
    public interface IMessageParser<out T>
    {
        T MessageParse(byte[] bytes);
    }
}