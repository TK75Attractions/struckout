namespace Struckout.Infrastructure
{
    public interface IMessageParser<T>
    {
        T MessageParse(byte[] bytes);
    }
}