namespace Struckout.Application
{
    public interface IMessageParser<T> 
    {
        T MessageParse(byte[] bytes);
    }
}